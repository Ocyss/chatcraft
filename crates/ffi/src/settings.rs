//! Native settings ABI. No Lua engine is created or called.
//! Handles must not be accessed concurrently, including free. Strings are UTF-8,
//! NUL-terminated; returned strings belong to chatcraft_string_free.
use super::{into_c_string, read_str};
use chatcraft_core::settings::SettingsStore;
use std::ffi::{c_char, c_void};

/// Open a configuration DIRECTORY (not a filename). Invalid UTF-8/null returns
/// null. Missing files use defaults; corrupt/unreadable files return a default
/// handle with last_error set, without changing the file.
/// # Safety
/// Non-null path must point to a live NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_open(path: *const c_char) -> *mut c_void {
    let Some(path) = read_str(path).filter(|path| !path.is_empty()) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(SettingsStore::open(std::path::Path::new(path)))).cast()
}

/// # Safety
/// Handle must be null or a live settings handle, freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_free(handle: *mut c_void) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle.cast::<SettingsStore>()) });
    }
}

/// # Safety
/// Handle must be null or a live settings handle with exclusive host access.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_get_json(handle: *mut c_void) -> *mut c_char {
    unsafe { handle.cast::<SettingsStore>().as_ref() }
        .map(|store| into_c_string(store.get_json()))
        .unwrap_or(std::ptr::null_mut())
}

/// Replace the full document, preserving in-memory state on any failure.
/// # Safety
/// Handle must be null or live and exclusively accessed. JSON must be null or a
/// valid NUL-terminated string. Preserve unknown fields when editing get_json.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_set_json(
    handle: *mut c_void,
    json: *const c_char,
) -> bool {
    let Some(store) = (unsafe { handle.cast::<SettingsStore>().as_mut() }) else {
        return false;
    };
    let Some(json) = read_str(json) else {
        store.last_error = Some("expected UTF-8 JSON".into());
        return false;
    };
    store.set_json(json)
}

/// Allocated error string, or null when no error (also null for a null handle).
/// A successful set clears the error. Reads do not clear it.
/// # Safety
/// Handle must be null or a live settings handle with exclusive host access.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_last_error(handle: *mut c_void) -> *mut c_char {
    unsafe { handle.cast::<SettingsStore>().as_ref() }
        .and_then(|store| store.last_error.clone())
        .map(into_c_string)
        .unwrap_or(std::ptr::null_mut())
}

/// Exact stable raw ID lookup; unknown IDs remain visible unless explicitly hidden.
/// # Safety
/// Handle must be null or live; ID must be null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_ui_visible(
    handle: *mut c_void,
    id: *const c_char,
) -> bool {
    match (
        unsafe { handle.cast::<SettingsStore>().as_ref() },
        read_str(id),
    ) {
        (Some(store), Some(id)) => store.settings.ui_visible(id),
        _ => true,
    }
}

/// mode: 0 main, 1 custom folder, 2 archive/other. Unknown modes never hide.
/// Native supplies membership in ANY custom folder, independent of folder ID.
/// # Safety
/// Handle must be null or a live settings handle with exclusive host access.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_chat_list_hide(
    handle: *mut c_void,
    enabled: bool,
    mode: u32,
    in_custom_folder: bool,
) -> bool {
    unsafe { handle.cast::<SettingsStore>().as_ref() }.is_some_and(|store| {
        store
            .settings
            .chat_list_hide(enabled, mode, in_custom_folder)
    })
}

/// Stable-sort descending by priority to put read chats first when enabled.
/// # Safety
/// Handle must be null or a live settings handle with exclusive host access.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_chat_list_priority(
    handle: *mut c_void,
    is_read: bool,
) -> i32 {
    unsafe { handle.cast::<SettingsStore>().as_ref() }
        .map_or(0, |store| store.settings.chat_list_priority(is_read))
}

/// # Safety
/// Handle must be null or a live settings handle with exclusive host access.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_settings_hide_sponsored(handle: *mut c_void) -> bool {
    unsafe { handle.cast::<SettingsStore>().as_ref() }
        .is_none_or(|store| store.settings.hide_sponsored)
}
