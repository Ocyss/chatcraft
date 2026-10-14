//! C ABI 接入层，供 Desktop(C++) / Android(JNI) / iOS 调用。
//!
//! 所有字符串均为 UTF-8 JSON。返回的字符串必须用 `chatcraft_string_free`
//! 释放；引擎句柄用 `chatcraft_engine_free` 释放。

use std::ffi::{CStr, CString, c_char};
use std::panic::AssertUnwindSafe;

use chatcraft_core::{Engine, EngineConfig, Message};

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

/// 创建引擎。`config_json` 为 `EngineConfig` 的 JSON；传 null 使用默认配置。
/// 失败返回 null。
///
/// # Safety
/// `config_json` 必须是合法的以 NUL 结尾的 UTF-8 字符串或 null。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_engine_new(config_json: *const c_char) -> *mut Engine {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let config: EngineConfig = match read_str(config_json) {
            Some(s) => serde_json::from_str(s).ok()?,
            None => EngineConfig::default(),
        };
        Engine::new(config).ok()
    }));
    match result {
        Ok(Some(engine)) => Box::into_raw(Box::new(engine)),
        _ => std::ptr::null_mut(),
    }
}

/// # Safety
/// `engine` 必须来自 `chatcraft_engine_new`，且只释放一次。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_engine_free(engine: *mut Engine) {
    if !engine.is_null() {
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// 过滤一条消息。`message_json` 为 `Message` 的 JSON。
/// 返回 `FilterOutcome` 的 JSON 字符串（调用方负责释放）；失败返回 null。
///
/// # Safety
/// `engine` 与 `message_json` 必须合法有效。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_filter_message(
    engine: *const Engine,
    message_json: *const c_char,
) -> *mut c_char {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let engine = unsafe { engine.as_ref() }?;
        let msg: Message = serde_json::from_str(read_str(message_json)?).ok()?;
        let outcome = engine.filter(&msg);
        serde_json::to_string(&outcome).ok()
    }));
    match result {
        Ok(Some(json)) => into_c_string(json),
        _ => std::ptr::null_mut(),
    }
}

/// 官方广告是否已禁用（环境变量 / 覆盖设置）。
#[unsafe(no_mangle)]
pub extern "C" fn chatcraft_ads_disabled() -> bool {
    chatcraft_core::ads::ads_disabled()
}

/// 显式覆盖广告开关（例如用户在设置页切换）。
#[unsafe(no_mangle)]
pub extern "C" fn chatcraft_set_ads_disabled(disabled: bool) {
    chatcraft_core::ads::set_ads_disabled_override(disabled);
}

/// # Safety
/// `s` 必须是本库返回的字符串指针。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}
