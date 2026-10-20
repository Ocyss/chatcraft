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
    /// Optional directory of scripts; direct `.lua` children load in path order.
    pub script_dir: Option<std::path::PathBuf>,
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

/// Load status of a script (or an unreadable script directory).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptStatus {
    pub path: std::path::PathBuf,
    pub name: String,
    pub loaded: bool,
    pub error: Option<String>,
}

/// Platform-independent UI identity; IDs must not depend on localized labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiItem {
    pub id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiFilterOutcome {
    pub hide: bool,
    pub source: String,
    pub reason: Option<String>,
}

pub struct Engine {
    rules: RuleSet,
    lua: Vec<LuaFilter>,
    scripts: Vec<ScriptStatus>,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let mut rules = config.rules;
        rules
            .compile()
            .map_err(|e| format!("rule compile error: {e}"))?;
        let mut lua = Vec::new();
        let mut scripts = Vec::new();
        let legacy = match &config.lua_script {
            Some(path) => Some(
                LuaFilter::from_file(path)
                    .map_err(|e| format!("lua load error ({}): {e}", path.display()))?,
            ),
            None => None,
        };
        if let Some(filter) = legacy {
            lua.push(filter);
            scripts.push(script_status(
                config.lua_script.as_ref().unwrap().clone(),
                None,
            ));
        }
        if let Some(dir) = config.script_dir {
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut paths = Vec::new();
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                let path = entry.path();
                                if path.extension().is_some_and(|ext| ext == "lua")
                                    && path.is_file()
                                {
                                    paths.push(path);
                                }
                            }
                            Err(error) => {
                                scripts.push(script_status(dir.clone(), Some(error.to_string())))
                            }
                        }
                    }
                    paths.sort();
                    for path in paths {
                        match LuaFilter::from_file(&path) {
                            Ok(filter) => {
                                lua.push(filter);
                                scripts.push(script_status(path, None));
                            }
                            Err(error) => {
                                scripts.push(script_status(path, Some(error.to_string())))
                            }
                        }
                    }
                }
                Err(error) => scripts.push(script_status(dir, Some(error.to_string()))),
            }
        }
        Ok(Self {
            rules,
            lua,
            scripts,
        })
    }

    /// Script load statuses, including failures that did not prevent startup.
    pub fn scripts_status(&self) -> &[ScriptStatus] {
        &self.scripts
    }

    /// Any script may hide an item; errors, absent callbacks and other actions allow it.
    pub fn filter_ui(&self, item: &UiItem) -> UiFilterOutcome {
        for lua in &self.lua {
            match lua.filter_ui(item) {
                Ok(Some((Action::Hide, reason))) => {
                    return UiFilterOutcome {
                        hide: true,
                        source: "lua".into(),
                        reason,
                    };
                }
                Err(error) => eprintln!("chatcraft lua UI error: {error}"),
                _ => {}
            }
        }
        UiFilterOutcome {
            hide: false,
            source: "default".into(),
            reason: None,
        }
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
        for lua in &self.lua {
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

fn script_status(path: std::path::PathBuf, error: Option<String>) -> ScriptStatus {
    ScriptStatus {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        loaded: error.is_none(),
        path,
        error,
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
