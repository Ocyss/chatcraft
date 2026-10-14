//! 引擎：组合广告开关、规则引擎、Lua 脚本，对消息给出最终动作。
//!
//! 注意：由于 Lua 运行时的限制，`Engine` 不是 `Send`，平台侧应在
//! 同一线程内创建和使用（如消息处理线程）。

use serde::{Deserialize, Serialize};

use crate::ads;
use crate::lua::LuaFilter;
use crate::message::Message;
use crate::rules::{Action, RuleSet};

/// 引擎配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineConfig {
    #[serde(default)]
    pub rules: RuleSet,
    /// Lua 脚本文件路径（可选）。
    pub lua_script: Option<std::path::PathBuf>,
}

/// 过滤结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterOutcome {
    pub action: Action,
    /// 命中的规则名或 "lua" / "ads"。
    pub source: String,
    pub reason: Option<String>,
}

impl FilterOutcome {
    fn allow() -> Self {
        Self {
            action: Action::Allow,
            source: "default".into(),
            reason: None,
        }
    }
}

pub struct Engine {
    rules: RuleSet,
    lua: Option<LuaFilter>,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let mut rules = config.rules;
        rules
            .compile()
            .map_err(|e| format!("rule compile error: {e}"))?;
        let lua = match &config.lua_script {
            Some(path) => Some(
                LuaFilter::from_file(path)
                    .map_err(|e| format!("lua load error ({}): {e}", path.display()))?,
            ),
            None => None,
        };
        Ok(Self { rules, lua })
    }

    /// 对单条消息求值。优先级：官方广告 > 声明式规则 > Lua 脚本。
    pub fn filter(&self, msg: &Message) -> FilterOutcome {
        if ads::should_drop_sponsored(msg.is_sponsored) {
            return FilterOutcome {
                action: Action::Hide,
                source: "ads".into(),
                reason: Some("sponsored message disabled".into()),
            };
        }
        if let Some((name, action)) = self.rules.evaluate(msg) {
            return FilterOutcome {
                action,
                source: name.to_string(),
                reason: None,
            };
        }
        if let Some(lua) = &self.lua {
            match lua.filter(msg) {
                Ok(Some((action, reason))) => {
                    return FilterOutcome {
                        action,
                        source: "lua".into(),
                        reason,
                    };
                }
                Ok(None) => {}
                Err(e) => eprintln!("chatcraft lua error: {e}"),
            }
        }
        FilterOutcome::allow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ChatType;
    use crate::rules::Rule;

    fn msg(text: &str) -> Message {
        Message {
            id: 1,
            chat_id: 1,
            chat_type: ChatType::Group,
            chat_title: None,
            sender_id: None,
            sender_name: None,
            text: text.into(),
            is_sponsored: false,
            is_forwarded: false,
        }
    }

    #[test]
    fn sponsored_hidden() {
        let e = Engine::new(EngineConfig::default()).unwrap();
        let mut m = msg("buy now");
        m.is_sponsored = true;
        assert_eq!(e.filter(&m).action, Action::Hide);
    }

    #[test]
    fn rules_before_allow() {
        let mut cfg = EngineConfig::default();
        cfg.rules.rules.push(Rule {
            name: "spam".into(),
            action: Action::Hide,
            chat_types: vec![],
            chat_ids: vec![],
            keywords: vec!["spam".into()],
            patterns: vec![],
            compiled: vec![],
        });
        let e = Engine::new(cfg).unwrap();
        assert_eq!(e.filter(&msg("spam!")).source, "spam");
        assert_eq!(e.filter(&msg("hi")).action, Action::Allow);
    }
}
