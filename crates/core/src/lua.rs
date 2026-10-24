//! Lua extensions: prefer `chatcraft.filter_message`, `filter_ui`, `filter_chat_list`.
//!
//! `msg` fields match the serialized [`Message`] structure.
//! 返回值可以是字符串 "allow" | "hide" | "collapse" | "highlight"，
//! 或表 `{ action = "hide", reason = "..." }`。

use mlua::{Lua, LuaSerdeExt, Value};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

pub const UI_REQUEST_LIMIT: usize = 128;

#[derive(Debug, Clone, serde::Serialize)]
pub struct UiRequest {
    pub id: u64,
    pub kind: &'static str,
    pub title: String,
    pub text: String,
}

#[derive(Clone, Default)]
pub(crate) struct UiQueue {
    next_id: u64,
    requests: VecDeque<UiRequest>,
    pending: usize,
}

impl UiQueue {
    pub(crate) fn drain(&mut self) -> Vec<UiRequest> {
        self.requests.drain(..).collect()
    }
}

type Callbacks = Rc<RefCell<HashMap<u64, mlua::RegistryKey>>>;

use crate::message::Message;
use crate::rules::Action;

/// 单条消息允许执行的最大 Lua 指令数，防止死循环拖垮 UI 线程。
const INSTRUCTION_LIMIT: u32 = 1_000_000;
const HOOK_INTERVAL: u32 = 1_000;

type Handlers = Rc<RefCell<HashMap<String, Vec<mlua::Function>>>>;

pub struct LuaFilter {
    lua: Lua,
    counter: std::rc::Rc<std::cell::Cell<u32>>,
    callbacks: Callbacks,
    handlers: Handlers,
    queue: Rc<RefCell<UiQueue>>,
}

impl LuaFilter {
    pub fn from_source(source: &str) -> mlua::Result<Self> {
        Self::with_queue(source, Rc::new(RefCell::new(UiQueue::default())))
    }

    pub(crate) fn with_queue(source: &str, queue: Rc<RefCell<UiQueue>>) -> mlua::Result<Self> {
        let lua = Lua::new();
        let counter = Self::install_instruction_limit(&lua);
        let callbacks: Callbacks = Rc::default();
        let api = lua.create_table()?;
        let handlers: Handlers = Rc::default();
        for hook in ["filter_message", "filter_ui", "filter_chat_list"] {
            let handlers = handlers.clone();
            api.set(
                format!("register_{hook}"),
                lua.create_function(move |_, args: mlua::MultiValue| {
                    let handler = match (args.len(), args.front()) {
                        (1, Some(Value::Function(handler))) => handler,
                        _ => {
                            return Err(mlua::Error::RuntimeError(
                                "registration requires exactly one function (use dot syntax)"
                                    .into(),
                            ));
                        }
                    };
                    handlers
                        .borrow_mut()
                        .entry(hook.into())
                        .or_default()
                        .push(handler.clone());
                    Ok(())
                })?,
            )?;
        }
        let utils = lua.create_table()?;
        for kind in ["alert", "confirm"] {
            let queue = queue.clone();
            let callbacks = callbacks.clone();
            utils.set(
                kind,
                lua.create_function(move |lua, args: mlua::MultiValue| {
                    let expected = if kind == "confirm" { 3 } else { 2 };
                    if args.len() != expected {
                        return Err(mlua::Error::RuntimeError(
                            "invalid UI utility arguments".into(),
                        ));
                    }
                    let title = match &args[0] {
                        Value::String(s) => s.to_str()?.to_string(),
                        _ => {
                            return Err(mlua::Error::RuntimeError("title must be a string".into()));
                        }
                    };
                    let text = match &args[1] {
                        Value::String(s) => s.to_str()?.to_string(),
                        _ => return Err(mlua::Error::RuntimeError("text must be a string".into())),
                    };
                    let mut queue = queue.borrow_mut();
                    let queued_alerts = queue.requests.iter().filter(|r| r.kind == "alert").count();
                    if queued_alerts + queue.pending >= UI_REQUEST_LIMIT {
                        return Err(mlua::Error::RuntimeError(
                            "UI request limit exceeded".into(),
                        ));
                    }
                    // Stay exactly representable in JavaScript/Qt JSON numbers.
                    if queue.next_id >= 9_007_199_254_740_991 {
                        return Err(mlua::Error::RuntimeError("UI request IDs exhausted".into()));
                    }
                    let id = queue.next_id + 1;
                    if kind == "confirm" {
                        let Value::Function(callback) = &args[2] else {
                            return Err(mlua::Error::RuntimeError(
                                "callback must be a function".into(),
                            ));
                        };
                        callbacks
                            .borrow_mut()
                            .insert(id, lua.create_registry_value(callback.clone())?);
                        queue.pending += 1;
                    }
                    queue.next_id = id;
                    queue.requests.push_back(UiRequest {
                        id,
                        kind,
                        title,
                        text,
                    });
                    Ok(())
                })?,
            )?;
        }
        api.set("utils", utils)?;
        lua.globals().set("chatcraft", api)?;
        counter.set(0);
        lua.load(source).exec()?;
        Ok(Self {
            lua,
            counter,
            callbacks,
            handlers,
            queue,
        })
    }

    pub(crate) fn respond(&self, id: u64, confirmed: bool) -> Option<mlua::Result<()>> {
        let key = self.callbacks.borrow_mut().remove(&id)?;
        {
            let mut queue = self.queue.borrow_mut();
            queue.pending -= 1;
            queue.requests.retain(|request| request.id != id);
        }
        let result = (|| {
            let callback: mlua::Function = self.lua.registry_value(&key)?;
            self.lua.remove_registry_value(key)?;
            self.counter.set(0);
            callback.call::<()>(confirmed)
        })();
        Some(result)
    }

    pub fn from_file(path: &std::path::Path) -> std::io::Result<Self> {
        let source = std::fs::read_to_string(path)?;
        Self::from_source(&source)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }

    fn install_instruction_limit(lua: &Lua) -> std::rc::Rc<std::cell::Cell<u32>> {
        use std::cell::Cell;
        let counter = std::rc::Rc::new(Cell::new(0u32));
        let hook_counter = counter.clone();
        lua.set_hook(
            mlua::HookTriggers::new().every_nth_instruction(HOOK_INTERVAL),
            move |_lua, _dbg| {
                let c = hook_counter.get().saturating_add(HOOK_INTERVAL);
                hook_counter.set(c);
                if c >= INSTRUCTION_LIMIT {
                    Err(mlua::Error::RuntimeError(
                        "lua instruction limit exceeded".into(),
                    ))
                } else {
                    Ok(mlua::VmState::Continue)
                }
            },
        )
        .expect("set_hook");
        counter
    }

    /// Call `filter(msg)`; missing callbacks and runtime errors fail open.
    pub fn filter(&self, msg: &Message) -> mlua::Result<Option<(Action, Option<String>)>> {
        self.call_filter("filter", msg)
    }

    /// Call `filter_ui(item)` with stable `id` and `kind` strings.
    /// Return `"hide"` or `{ action = "hide", reason = "..." }` to hide.
    pub fn filter_ui(
        &self,
        item: &crate::engine::UiItem,
    ) -> mlua::Result<Option<(Action, Option<String>)>> {
        self.call_filter("filter_ui", item)
    }

    /// Metadata is optional; non-string fields are ignored.
    pub fn metadata(&self) -> crate::engine::ScriptMetadata {
        let mut metadata = crate::engine::ScriptMetadata::default();
        if let Ok(Value::Table(table)) = self.lua.globals().get::<Value>("script") {
            let string = |key| match table.raw_get::<Value>(key) {
                Ok(Value::String(s)) => s.to_str().ok().map(|s| s.to_string()),
                _ => None,
            };
            metadata.name = string("name");
            metadata.version = string("version");
            metadata.description = string("description");
            metadata.author = string("author");
            metadata.update_url = string("update_url");
        }
        metadata
    }

    /// Namespaced callbacks take precedence; nil does not invoke the legacy callback.
    pub fn call(&self, name: &str, item: &impl serde::Serialize) -> mlua::Result<Value> {
        self.counter.set(0);
        let (hook, legacy) = match name {
            "filter" => ("filter_message", "on_message"),
            "filter_ui" => ("filter_ui", "on_ui"),
            "on_chat" => ("filter_chat_list", "on_chat"),
            other => (other, other),
        };
        let globals = self.lua.globals();
        let namespaced = match globals.raw_get::<Value>("chatcraft")? {
            Value::Table(table) => {
                let current = table.raw_get::<Value>(hook)?;
                if matches!(current, Value::Function(_)) {
                    current
                } else {
                    table.raw_get::<Value>(legacy)?
                }
            }
            _ => Value::Nil,
        };
        let callback = if matches!(namespaced, Value::Function(_)) {
            namespaced
        } else if name == "filter" || name == "filter_ui" {
            globals.raw_get::<Value>(name)?
        } else {
            Value::Nil
        };
        // Snapshot registrations so callbacks may register handlers for future calls.
        let mut handlers = self
            .handlers
            .borrow()
            .get(hook)
            .cloned()
            .unwrap_or_default();
        if handlers.is_empty() {
            return match callback {
                Value::Function(f) => f.call(self.lua.to_value(item)?),
                _ => Ok(Value::Nil),
            };
        }
        if let Value::Function(f) = callback {
            handlers.push(f);
        }
        let mut first = Value::Nil;
        let mut chat: Option<crate::engine::ChatListOutcome> = None;
        for handler in handlers {
            self.counter.set(0);
            let value = match handler.call::<Value>(self.lua.to_value(item)?) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("chatcraft lua handler error: {error}");
                    continue;
                }
            };
            if hook == "filter_chat_list" {
                if let Ok(result) = self.decode::<crate::engine::ChatListOutcome>(value) {
                    if let Some(outcome) = &mut chat {
                        outcome.hide |= result.hide;
                    } else {
                        chat = Some(result);
                    }
                }
            } else if let Some((action, _)) = decode_action(&value) {
                if hook == "filter_message" || action == Action::Hide {
                    return Ok(value);
                }
                if matches!(first, Value::Nil) {
                    first = value;
                }
            }
        }
        match chat {
            Some(outcome) => self.lua.to_value(&outcome),
            None => Ok(first),
        }
    }

    pub fn decode<T: serde::de::DeserializeOwned>(&self, value: Value) -> mlua::Result<T> {
        self.lua.from_value(value)
    }

    fn call_filter(
        &self,
        name: &str,
        item: &impl serde::Serialize,
    ) -> mlua::Result<Option<(Action, Option<String>)>> {
        self.counter.set(0);
        match self.call(name, item) {
            Ok(Value::String(s)) => Ok(parse_action(s.to_str()?.as_ref()).map(|a| (a, None))),
            Ok(Value::Table(t)) => {
                let action: Option<String> = t.get("action").ok();
                let reason: Option<String> = t.get("reason").ok();
                Ok(action.and_then(|a| parse_action(&a)).map(|a| (a, reason)))
            }
            Ok(_) => Ok(None),
            Err(e) => {
                eprintln!("chatcraft lua filter error: {e}");
                Ok(None)
            }
        }
    }
}

fn decode_action(value: &Value) -> Option<(Action, Option<String>)> {
    match value {
        Value::String(s) => parse_action(s.to_str().ok()?.as_ref()).map(|a| (a, None)),
        Value::Table(t) => {
            let action: String = t.get("action").ok()?;
            let reason: Option<String> = t.get("reason").ok();
            parse_action(&action).map(|a| (a, reason))
        }
        _ => None,
    }
}

fn parse_action(s: &str) -> Option<Action> {
    match s.to_ascii_lowercase().as_str() {
        "allow" => Some(Action::Allow),
        "hide" => Some(Action::Hide),
        "collapse" => Some(Action::Collapse),
        "highlight" => Some(Action::Highlight),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ChatType;

    fn msg(text: &str) -> Message {
        Message {
            id: 1,
            chat_id: 7,
            chat_type: ChatType::Channel,
            chat_title: Some("news".into()),
            sender_id: Some(3),
            sender_name: Some("bot".into()),
            text: text.into(),
            is_sponsored: false,
            is_forwarded: false,
        }
    }

    #[test]
    fn lua_filter_hides() {
        let f = LuaFilter::from_source(
            r#"
            function filter(msg)
                if msg.chat_type == "channel" and msg.text:find("推广") then
                    return { action = "hide", reason = "channel spam" }
                end
                return "allow"
            end
            "#,
        )
        .unwrap();
        let r = f.filter(&msg("这是一条推广")).unwrap();
        assert_eq!(r, Some((Action::Hide, Some("channel spam".into()))));
        assert_eq!(
            f.filter(&msg("早上好")).unwrap(),
            Some((Action::Allow, None))
        );
    }

    #[test]
    fn infinite_loop_is_aborted() {
        let f = LuaFilter::from_source("function filter(msg) while true do end end").unwrap();
        assert!(f.filter(&msg("x")).unwrap().is_none());
    }
}
