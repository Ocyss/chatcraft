//! Lua 脚本扩展：脚本定义全局函数 `filter(msg)` 决定消息动作。
//!
//! 脚本示例见 `scripts/example.lua`。`msg` 字段与 [`Message`] 序列化结构一致。
//! 返回值可以是字符串 "allow" | "hide" | "collapse" | "highlight"，
//! 或表 `{ action = "hide", reason = "..." }`。

use mlua::{Lua, LuaSerdeExt, Value};

use crate::message::Message;
use crate::rules::Action;

/// 单条消息允许执行的最大 Lua 指令数，防止死循环拖垮 UI 线程。
const INSTRUCTION_LIMIT: u32 = 1_000_000;

pub struct LuaFilter {
    lua: Lua,
    counter: std::rc::Rc<std::cell::Cell<u32>>,
}

impl LuaFilter {
    pub fn from_source(source: &str) -> mlua::Result<Self> {
        let lua = Lua::new();
        let counter = Self::install_instruction_limit(&lua);
        lua.load(source).exec()?;
        Ok(Self { lua, counter })
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
            mlua::HookTriggers::new().every_nth_instruction(1),
            move |_lua, _dbg| {
                let c = hook_counter.get() + 1;
                hook_counter.set(c);
                if c > INSTRUCTION_LIMIT {
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

    /// 调用脚本的 `filter(msg)`；脚本未定义或出错时返回 Ok(None)（视为 allow）。
    pub fn filter(&self, msg: &Message) -> mlua::Result<Option<(Action, Option<String>)>> {
        self.counter.set(0);
        let globals = self.lua.globals();
        let f: Value = globals.get("filter")?;
        let Value::Function(f) = f else {
            return Ok(None);
        };
        let arg = self.lua.to_value(msg)?;
        match f.call::<Value>(arg) {
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
