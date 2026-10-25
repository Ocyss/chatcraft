use chatcraft_core::settings::SettingsStore;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "chatcraft-settings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn defaults_roundtrip_unknown_fields_ids_and_replacement() {
    let directory = Directory::new();
    let mut store = SettingsStore::open(&directory.0);
    assert!(!store.settings.hide_grouped_chats);
    assert!(!store.settings.read_first);
    assert!(store.settings.hide_sponsored);
    assert!(store.set_json(
        r#"{"version":1,"hidden_ui_ids":["future.z","raw.id","future.z"],"future":{"x":42}}"#
    ));
    let reopened = SettingsStore::open(&directory.0);
    assert_eq!(store.settings, reopened.settings);
    assert_eq!(
        reopened
            .settings
            .hidden_ui_ids
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        ["future.z", "raw.id"]
    );
    assert_eq!(reopened.settings.extra["future"]["x"], 42);
    assert!(!reopened.settings.ui_visible("future.z"));
    assert!(reopened.settings.ui_visible("unknown"));
    assert!(reopened.settings.ui_visible("RAW.ID"));
    // A second save replaces an existing destination, including on Windows.
    assert!(store.set_json(&store.get_json()));
    assert_eq!(SettingsStore::open(&directory.0).settings, store.settings);
}

#[test]
fn failed_save_and_invalid_json_leave_memory_and_disk_unchanged() {
    let directory = Directory::new();
    let mut store = SettingsStore::open(&directory.0);
    assert!(store.set_json("{}"));
    let before = store.get_json();
    let disk = std::fs::read(directory.0.join("settings.json")).unwrap();
    assert!(!store.set_json(r#"{"read_first":"bad"}"#));
    assert_eq!(store.get_json(), before);
    assert_eq!(
        std::fs::read(directory.0.join("settings.json")).unwrap(),
        disk
    );
    // Deterministic replacement failure, without relying on permission bits.
    std::fs::remove_file(directory.0.join("settings.json")).unwrap();
    std::fs::create_dir(directory.0.join("settings.json")).unwrap();
    assert!(!store.set_json(r#"{"read_first":true}"#));
    assert!(store.last_error.is_some());
    assert_eq!(store.get_json(), before);
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn malformed_and_future_configs_fall_back_without_overwriting() {
    let directory = Directory::new();
    for json in [
        "broken",
        r#"{"version":99,"future":true}"#,
        r#"{"hidden_ui_ids":false}"#,
    ] {
        std::fs::write(directory.0.join("settings.json"), json).unwrap();
        let mut store = SettingsStore::open(&directory.0);
        assert!(store.last_error.is_some());
        assert!(!store.settings.read_first);
        assert!(store.settings.ui_visible("anything"));
        assert_eq!(
            std::fs::read_to_string(directory.0.join("settings.json")).unwrap(),
            json
        );
        if json.contains("99") {
            assert!(!store.set_json("{}"));
            assert_eq!(
                std::fs::read_to_string(directory.0.join("settings.json")).unwrap(),
                json
            );
        }
    }
}

#[test]
fn any_custom_folder_only_hides_in_main_and_priority_is_typed() {
    let directory = Directory::new();
    let mut store = SettingsStore::open(&directory.0);
    assert!(!store.settings.chat_list_hide(true, 0, true));
    assert_eq!(store.settings.chat_list_priority(true), 0);
    assert!(store.set_json(r#"{"hide_grouped_chats":true,"read_first":true}"#));
    for folder_ids in [vec![3], vec![999], vec![1, 77], vec![]] {
        let membership = !folder_ids.is_empty();
        for mode in [0, 1, 2, 99] {
            assert_eq!(
                store.settings.chat_list_hide(true, mode, membership),
                mode == 0 && membership
            );
            assert!(!store.settings.chat_list_hide(false, mode, membership));
        }
    }
    assert_eq!(store.settings.chat_list_priority(true), 1);
    assert_eq!(store.settings.chat_list_priority(false), 0);
}
