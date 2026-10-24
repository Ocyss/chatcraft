//! C ABI 接入层，供 Desktop(C++) / Android(JNI) / iOS 调用。
//!
//! 所有字符串均为 UTF-8 JSON。返回的字符串必须用 `chatcraft_string_free`
//! 释放；引擎句柄用 `chatcraft_engine_free` 释放。
//!
//! Async UI protocol for desktop hosts:
//! - Lua `chatcraft.utils.alert(title, text)` and
//!   `chatcraft.utils.confirm(title, text, callback)` only enqueue, returning nil.
//! - Poll `chatcraft_ui_requests` after loading, filtering, lifecycle calls and
//!   responses. It drains a FIFO array of `{id,kind,title,text}`; `kind` is
//!   `alert` or `confirm`. IDs are positive integers unique within this engine,
//!   bounded to 2^53-1. Never route an old engine's response to a replacement.
//! - Show dialogs on the Qt UI thread; send confirm acceptance as true and
//!   rejection/close as false via `chatcraft_ui_respond` on the engine thread.
//!   Alerts do not accept responses. Keep polling to receive callback requests.
//! - Capacity is 128 queued alerts plus outstanding confirms (including drained
//!   confirms). Overflow/invalid utility arguments raise a Lua error and do not
//!   enqueue. Filter errors fail open; failed script loads discard their requests.
//! - Responding consumes the callback before invocation, resets its instruction
//!   budget, and releases its registry entry. Callback errors return ok:false;
//!   unknown/duplicate IDs also return ok:false. Closing/freeing an engine drops
//!   all outstanding callbacks. All engine operations must stay on its creating
//!   thread, without concurrent or reentrant host calls.

use std::ffi::{CStr, CString, c_char};
use std::panic::AssertUnwindSafe;

use chatcraft_core::{Engine, EngineConfig, Message, UiItem};

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

/// Return script load statuses as a JSON array; free with `chatcraft_string_free`.
/// Returns null for a null engine or serialization failure.
///
/// # Safety
/// `engine` must be null or a live handle used on its creating thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_scripts_status(engine: *const Engine) -> *mut c_char {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let engine = unsafe { engine.as_ref() }?;
        serde_json::to_string(engine.scripts_status()).ok()
    }));
    match result {
        Ok(Some(json)) => into_c_string(json),
        _ => std::ptr::null_mut(),
    }
}

/// Filter `{ "id": "stable.id", "kind": "button" }`, returning
/// `{ "hide": false, "source": "default", "reason": null }` JSON.
/// Free the result with `chatcraft_string_free`; invalid input returns null.
///
/// # Safety
/// `engine` must be null or a live handle used on its creating thread.
/// `item_json` must be null or a valid NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_filter_ui(
    engine: *const Engine,
    item_json: *const c_char,
) -> *mut c_char {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let engine = unsafe { engine.as_ref() }?;
        let item: UiItem = serde_json::from_str(read_str(item_json)?).ok()?;
        serde_json::to_string(&engine.filter_ui(&item)).ok()
    }));
    match result {
        Ok(Some(json)) => into_c_string(json),
        _ => std::ptr::null_mut(),
    }
}

/// Filter a ChatListItem JSON; returns ChatListOutcome JSON or null.
///
/// # Safety
/// Handle must be live on its creating thread; input must be null or a valid
/// NUL-terminated UTF-8 string. Free returned strings with chatcraft_string_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_filter_chat(
    engine: *const Engine,
    input: *const c_char,
) -> *mut c_char {
    json_request(engine, input, |engine, value| {
        let item: chatcraft_core::ChatListItem = serde_json::from_value(value).ok()?;
        serde_json::to_value(engine.filter_chat(&item)).ok()
    })
}

/// Filter an array of ChatListItem objects, preserving input order.
/// Invalid batches return null without evaluating any callbacks.
///
/// # Safety
/// Same handle, input and ownership requirements as chatcraft_filter_chat.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_filter_chats(
    engine: *const Engine,
    input: *const c_char,
) -> *mut c_char {
    json_request(engine, input, |engine, value| {
        let items: Vec<chatcraft_core::ChatListItem> = serde_json::from_value(value).ok()?;
        let outcomes: Vec<_> = items.iter().map(|item| engine.filter_chat(item)).collect();
        serde_json::to_value(outcomes).ok()
    })
}

/// Input is {"event":"startup"} or {"event":"manager_open"}.
/// Returns an array of ScriptPopup objects; repeated startup returns [].
///
/// # Safety
/// Same handle, input and ownership requirements as chatcraft_filter_chat.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_lifecycle(
    engine: *const Engine,
    input: *const c_char,
) -> *mut c_char {
    json_request(engine, input, |engine, value| {
        let event = serde_json::from_value(value.get("event")?.clone()).ok()?;
        serde_json::to_value(engine.lifecycle(event)).ok()
    })
}

/// Drain queued alerts and confirms as [{id,kind,title,text}]. Empty queue: [].
/// Pending confirms survive draining; alerts need no response.
///
/// # Safety
/// Handle must be null or live on its creating thread. Free returned JSON with
/// chatcraft_string_free. Null handle or internal failure returns null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_ui_requests(engine: *const Engine) -> *mut c_char {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let engine = unsafe { engine.as_ref() }?;
        serde_json::to_string(&engine.ui_requests()).ok()
    }));
    match result {
        Ok(Some(json)) => into_c_string(json),
        _ => std::ptr::null_mut(),
    }
}

/// Respond with {"id": positive_integer, "confirmed": boolean}.
/// Returns {"ok":true} or {"ok":false,"error":"..."}. Invalid input/handle
/// returns null. Callback errors consume the ID; duplicate/unknown IDs fail.
/// Callbacks execute synchronously, may enqueue more requests, and are bounded
/// by a fresh instruction budget. Never call concurrently or from a Qt UI
/// thread different from the engine's creating thread; marshal responses there.
///
/// # Safety
/// Same handle, input and string ownership requirements as chatcraft_filter_chat.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chatcraft_ui_respond(
    engine: *const Engine,
    input: *const c_char,
) -> *mut c_char {
    json_request(engine, input, |engine, value| {
        let id = value.get("id")?.as_u64()?;
        let confirmed = value.get("confirmed")?.as_bool()?;
        Some(match engine.ui_respond(id, confirmed) {
            Ok(()) => serde_json::json!({"ok":true}),
            Err(error) => serde_json::json!({"ok":false,"error":error}),
        })
    })
}

fn json_request(
    engine: *const Engine,
    input: *const c_char,
    callback: impl FnOnce(&Engine, serde_json::Value) -> Option<serde_json::Value>,
) -> *mut c_char {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let engine = unsafe { engine.as_ref() }?;
        let input = serde_json::from_str(read_str(input)?).ok()?;
        serde_json::to_string(&callback(engine, input)?).ok()
    }));
    match result {
        Ok(Some(json)) => into_c_string(json),
        _ => std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn take_json(ptr: *mut c_char) -> serde_json::Value {
        assert!(!ptr.is_null());
        let json = serde_json::from_str(unsafe { CStr::from_ptr(ptr) }.to_str().unwrap()).unwrap();
        unsafe { chatcraft_string_free(ptr) };
        json
    }

    #[test]
    fn asynchronous_ui_protocol() {
        let path =
            std::env::temp_dir().join(format!("chatcraft-ffi-ui-{}.lua", std::process::id()));
        std::fs::write(
            &path,
            r#"
            chatcraft.utils.alert('notice', 'hello')
            chatcraft.utils.confirm('question', 'continue?', function(confirmed)
                chatcraft.utils.alert('answer', tostring(confirmed))
            end)
        "#,
        )
        .unwrap();
        let config = CString::new(serde_json::json!({"lua_script":path}).to_string()).unwrap();
        unsafe {
            let engine = chatcraft_engine_new(config.as_ptr());
            assert!(!engine.is_null());
            let requests = take_json(chatcraft_ui_requests(engine));
            assert_eq!(requests.as_array().unwrap().len(), 2);
            assert_eq!(requests[0]["kind"], "alert");
            assert_eq!(requests[1]["kind"], "confirm");
            assert!(requests[1]["id"].as_u64().is_some());
            assert_eq!(
                take_json(chatcraft_ui_requests(engine)),
                serde_json::json!([])
            );
            for invalid in [
                "{",
                "{}",
                r#"{"id":1}"#,
                r#"{"id":2,"confirmed":1}"#,
                r#"{"id":-1,"confirmed":true}"#,
            ] {
                let input = CString::new(invalid).unwrap();
                assert!(chatcraft_ui_respond(engine, input.as_ptr()).is_null());
            }
            let input = CString::new(
                serde_json::json!({"id":requests[1]["id"],"confirmed":false}).to_string(),
            )
            .unwrap();
            assert_eq!(
                take_json(chatcraft_ui_respond(engine, input.as_ptr())),
                serde_json::json!({"ok":true})
            );
            let requests = take_json(chatcraft_ui_requests(engine));
            assert_eq!(requests[0]["text"], "false");
            let duplicate = take_json(chatcraft_ui_respond(engine, input.as_ptr()));
            assert_eq!(duplicate["ok"], false);
            assert!(duplicate["error"].is_string());
            assert!(chatcraft_ui_requests(std::ptr::null()).is_null());
            assert!(chatcraft_ui_respond(std::ptr::null(), input.as_ptr()).is_null());
            assert!(chatcraft_ui_respond(engine, std::ptr::null()).is_null());
            chatcraft_engine_free(engine);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn chat_lifecycle_protocol_and_invalid_input() {
        unsafe {
            let engine = chatcraft_engine_new(std::ptr::null());
            let chat = CString::new(r#"{"chat_id":1,"chat_type":"group","title":"x","folder_ids":[2],"folder_count":1,"folder_id":2,"unread":false}"#).unwrap();
            assert_eq!(
                take_json(chatcraft_filter_chat(engine, chat.as_ptr())),
                serde_json::json!({"hide":false,"priority":0})
            );
            for event in ["startup", "startup", "manager_open"] {
                let input = CString::new(serde_json::json!({"event":event}).to_string()).unwrap();
                assert_eq!(
                    take_json(chatcraft_lifecycle(engine, input.as_ptr())),
                    serde_json::json!([])
                );
            }
            for invalid in ["{", "{}", r#"{"event":"unknown"}"#] {
                let input = CString::new(invalid).unwrap();
                assert!(chatcraft_filter_chat(engine, input.as_ptr()).is_null());
                assert!(chatcraft_lifecycle(engine, input.as_ptr()).is_null());
            }
            assert!(chatcraft_filter_chat(engine, std::ptr::null()).is_null());
            assert!(chatcraft_lifecycle(std::ptr::null(), chat.as_ptr()).is_null());
            chatcraft_engine_free(engine);
        }
    }

    #[test]
    fn chat_batch_order_validation_and_callbacks() {
        let path = std::env::temp_dir().join(format!(
            "chatcraft-ffi-chat-batch-{}.lua",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "local count = 0; function chatcraft.filter_chat_list(chat) count = count + 1; return {hide = chat.chat_id == 2, priority = count} end",
        ).unwrap();
        let config = CString::new(serde_json::json!({"lua_script": path}).to_string()).unwrap();
        unsafe {
            let engine = chatcraft_engine_new(config.as_ptr());
            assert!(!engine.is_null());
            assert_eq!(
                take_json(chatcraft_scripts_status(engine))[0]["loaded"],
                true
            );
            let item = serde_json::json!({"chat_id":1,"chat_type":"group","title":"one","folder_count":0,"folder_id":0});
            let invalid = CString::new(serde_json::json!([item, {}]).to_string()).unwrap();
            assert!(chatcraft_filter_chats(engine, invalid.as_ptr()).is_null());
            let input = CString::new(
                serde_json::json!([
                    item,
                    {"chat_id":2,"chat_type":"channel","title":"two","folder_count":0,"folder_id":0},
                    item
                ])
                .to_string(),
            )
            .unwrap();
            assert_eq!(
                take_json(chatcraft_filter_chats(engine, input.as_ptr())),
                serde_json::json!([
                    {"hide":false,"priority":1},
                    {"hide":true,"priority":2},
                    {"hide":false,"priority":3}
                ])
            );
            let empty = CString::new("[]").unwrap();
            assert_eq!(
                take_json(chatcraft_filter_chats(engine, empty.as_ptr())),
                serde_json::json!([])
            );
            for invalid in ["{", "{}", "null", "[null]"] {
                let input = CString::new(invalid).unwrap();
                assert!(chatcraft_filter_chats(engine, input.as_ptr()).is_null());
            }
            assert!(chatcraft_filter_chats(engine, std::ptr::null()).is_null());
            assert!(chatcraft_filter_chats(std::ptr::null(), input.as_ptr()).is_null());
            chatcraft_engine_free(engine);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn ui_and_status_json_roundtrip() {
        let path = std::env::temp_dir().join(format!("chatcraft-ffi-{}.lua", std::process::id()));
        std::fs::write(&path, "function filter_ui(item) if item.id == 'toolbar.ads' and item.kind == 'button' then return {action='hide', reason='ads'} end end").unwrap();
        let config = CString::new(serde_json::json!({"lua_script": path}).to_string()).unwrap();
        unsafe {
            let engine = chatcraft_engine_new(config.as_ptr());
            assert!(!engine.is_null());
            let statuses = take_json(chatcraft_scripts_status(engine));
            assert_eq!(statuses[0]["loaded"], true);
            assert_eq!(statuses[0]["error"], serde_json::Value::Null);
            let item = CString::new(r#"{"id":"toolbar.ads","kind":"button"}"#).unwrap();
            let outcome = take_json(chatcraft_filter_ui(engine, item.as_ptr()));
            assert_eq!(outcome["hide"], true);
            assert_eq!(outcome["reason"], "ads");
            let item = CString::new(r#"{"id":"toolbar.settings","kind":"button"}"#).unwrap();
            assert_eq!(
                take_json(chatcraft_filter_ui(engine, item.as_ptr()))["hide"],
                false
            );
            for invalid in ["{", "{}", r#"{"id":1,"kind":"button"}"#] {
                let input = CString::new(invalid).unwrap();
                assert!(chatcraft_filter_ui(engine, input.as_ptr()).is_null());
            }
            assert!(chatcraft_filter_ui(engine, std::ptr::null()).is_null());
            assert!(chatcraft_scripts_status(std::ptr::null()).is_null());
            assert!(chatcraft_filter_ui(std::ptr::null(), item.as_ptr()).is_null());
            chatcraft_engine_free(engine);
        }
        std::fs::remove_file(path).unwrap();
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
