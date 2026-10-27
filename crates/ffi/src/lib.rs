//! C ABI 接入层，供 Desktop(C++) / Android(JNI) / iOS 调用。
//!
//! 所有字符串均为 UTF-8。返回的字符串必须用 `chatcraft_string_free`
//! 释放；设置句柄用 `chatcraft_settings_free` 释放。

use std::ffi::{CStr, CString, c_char};

mod settings;
pub use settings::*;

fn read_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

fn into_c_string(s: String) -> *mut c_char {
    CString::new(s)
        .unwrap_or_else(|_| CString::new("{}").unwrap())
        .into_raw()
}

/// # Safety
/// `s` 必须是本库返回的字符串指针。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}
