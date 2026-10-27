use chatcraft_ffi::*;
use std::ffi::{CStr, CString};

#[test]
fn settings_abi_roundtrip_errors_and_null_defaults() {
    let directory =
        std::env::temp_dir().join(format!("chatcraft-settings-ffi-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = CString::new(directory.to_str().unwrap()).unwrap();
    unsafe {
        assert!(chatcraft_settings_open(std::ptr::null()).is_null());
        assert!(chatcraft_settings_open(c"".as_ptr()).is_null());
        let handle = chatcraft_settings_open(path.as_ptr());
        assert!(!handle.is_null());
        assert!(chatcraft_settings_hide_sponsored(handle));
        assert!(chatcraft_settings_set_json(handle, c"{\"hidden_ui_ids\":[\"future.id\"],\"hide_grouped_chats\":true,\"read_first\":true,\"hide_sponsored\":false}".as_ptr()));
        assert!(!chatcraft_settings_ui_visible(
            handle,
            c"future.id".as_ptr()
        ));
        assert!(chatcraft_settings_ui_visible(handle, c"unknown".as_ptr()));
        assert!(chatcraft_settings_chat_list_hide(handle, true, 0, true));
        assert!(!chatcraft_settings_chat_list_hide(handle, true, 1, true));
        assert_eq!(chatcraft_settings_chat_list_priority(handle, true), 1);
        assert!(!chatcraft_settings_hide_sponsored(handle));
        let json = chatcraft_settings_get_json(handle);
        assert!(CStr::from_ptr(json).to_str().unwrap().contains("future.id"));
        chatcraft_string_free(json);
        assert!(!chatcraft_settings_set_json(handle, c"invalid".as_ptr()));
        let error = chatcraft_settings_last_error(handle);
        assert!(!error.is_null());
        chatcraft_string_free(error);
        assert!(!chatcraft_settings_ui_visible(
            handle,
            c"future.id".as_ptr()
        ));
        assert!(chatcraft_settings_set_json(
            handle,
            c"{\"hide_sponsored\":true}".as_ptr()
        ));
        assert!(chatcraft_settings_hide_sponsored(handle));
        assert!(chatcraft_settings_set_json(handle, c"{}".as_ptr()));
        assert!(chatcraft_settings_last_error(handle).is_null());
        chatcraft_settings_free(handle);
        let reopened = chatcraft_settings_open(path.as_ptr());
        assert!(chatcraft_settings_hide_sponsored(reopened));
        chatcraft_settings_free(reopened);
        chatcraft_settings_free(std::ptr::null_mut());
        chatcraft_string_free(std::ptr::null_mut());
        assert!(chatcraft_settings_hide_sponsored(std::ptr::null_mut()));
        assert!(chatcraft_settings_get_json(std::ptr::null_mut()).is_null());
        assert!(chatcraft_settings_last_error(std::ptr::null_mut()).is_null());
        assert!(!chatcraft_settings_chat_list_hide(
            std::ptr::null_mut(),
            true,
            0,
            true
        ));
        assert!(chatcraft_settings_ui_visible(
            std::ptr::null_mut(),
            c"id".as_ptr()
        ));
    }
    std::fs::remove_dir_all(directory).unwrap();
}
