//! Built-in settings, independent of the Lua engine.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub hide_grouped_chats: bool,
    pub read_first: bool,
    pub hide_sponsored: bool,
    pub hidden_ui_ids: BTreeSet<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            hide_grouped_chats: false,
            read_first: false,
            hide_sponsored: true,
            hidden_ui_ids: BTreeSet::new(),
            extra: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn ui_visible(&self, id: &str) -> bool {
        !self.hidden_ui_ids.contains(id)
    }

    /// `mode`: 0 = main list, 1 = custom folder, 2 = archive/other.
    /// Membership means membership in ANY custom folder, not a fixed folder ID.
    pub fn chat_list_hide(&self, enabled: bool, mode: u32, in_custom_folder: bool) -> bool {
        enabled && self.hide_grouped_chats && mode == 0 && in_custom_folder
    }

    pub fn chat_list_priority(&self, is_read: bool) -> i32 {
        i32::from(self.read_first && is_read)
    }
}

pub struct SettingsStore {
    path: PathBuf,
    pub settings: Settings,
    pub last_error: Option<String>,
    // Never overwrite a format this version cannot interpret.
    unsupported_version: bool,
}

impl SettingsStore {
    pub fn open(directory: &Path) -> Self {
        let path = directory.join("settings.json");
        let mut store = Self {
            path,
            settings: Settings::default(),
            last_error: None,
            unsupported_version: false,
        };
        match fs::read_to_string(&store.path) {
            Ok(json) => match parse(&json) {
                Ok(settings) => store.settings = settings,
                Err(error) => {
                    store.unsupported_version = serde_json::from_str::<serde_json::Value>(&json)
                        .ok()
                        .and_then(|v| v.get("version").cloned())
                        .is_some_and(|v| v != serde_json::json!(1));
                    store.last_error = Some(error);
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => store.last_error = Some(error.to_string()),
        }
        store
    }

    pub fn get_json(&self) -> String {
        serde_json::to_string(&self.settings).expect("settings are JSON values")
    }

    /// Full-document replacement. Callers must retain unknown fields from get_json.
    pub fn set_json(&mut self, json: &str) -> bool {
        let result = (|| {
            if self.unsupported_version {
                return Err("unsupported settings version; refusing to overwrite".to_owned());
            }
            let next = parse(json)?;
            let bytes = serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?;
            atomic_save(&self.path, &bytes).map_err(|e| e.to_string())?;
            self.settings = next;
            Ok(())
        })();
        self.last_error = result.err();
        self.last_error.is_none()
    }
}

fn parse(json: &str) -> Result<Settings, String> {
    let settings: Settings = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if settings.version != 1 {
        return Err(format!("unsupported settings version {}", settings.version));
    }
    Ok(settings)
}

fn atomic_save(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = path.parent().expect("settings have a parent directory");
    fs::create_dir_all(directory)?;
    let (temporary, mut file) = loop {
        let temporary = directory.join(format!(
            ".settings-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(not(windows))]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
    }
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // REPLACE_EXISTING | WRITE_THROUGH; never delete the old file first.
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0x1 | 0x8) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
