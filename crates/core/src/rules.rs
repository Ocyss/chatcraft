//! 声明式规则引擎：关键词 / 正则匹配，按会话类型、会话 ID 过滤。

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::message::{ChatType, Message};

/// 规则命中后的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Hide,
    Collapse,
    Highlight,
}

impl Default for Action {
    fn default() -> Self {
        Action::Allow
    }
}

/// 一条过滤规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    #[serde(default)]
    pub action: Action,
    /// 限制生效的会话类型；为空表示全部。
    #[serde(default)]
    pub chat_types: Vec<ChatType>,
    /// 限制生效的会话 ID；为空表示全部。
    #[serde(default)]
    pub chat_ids: Vec<i64>,
    /// 关键词（不区分大小写，任一命中即可）。
    #[serde(default)]
    pub keywords: Vec<String>,
    /// 正则表达式（任一命中即可）。
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(skip)]
    #[doc(hidden)]
    pub compiled: Vec<Regex>,
}

impl Rule {
    pub fn compile(&mut self) -> Result<(), regex::Error> {
        self.compiled = self
            .patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    fn scope_matches(&self, msg: &Message) -> bool {
        (self.chat_types.is_empty() || self.chat_types.contains(&msg.chat_type))
            && (self.chat_ids.is_empty() || self.chat_ids.contains(&msg.chat_id))
    }

    fn body_matches(&self, msg: &Message) -> bool {
        let lower = msg.text.to_lowercase();
        if self
            .keywords
            .iter()
            .any(|k| lower.contains(&k.to_lowercase()))
        {
            return true;
        }
        self.compiled.iter().any(|re| re.is_match(&msg.text))
    }

    pub fn matches(&self, msg: &Message) -> bool {
        self.scope_matches(msg) && self.body_matches(msg)
    }
}

/// 规则集：按顺序匹配，第一条命中的规则决定动作。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl RuleSet {
    pub fn compile(&mut self) -> Result<(), regex::Error> {
        for r in &mut self.rules {
            r.compile()?;
        }
        Ok(())
    }

    /// 返回命中规则的 (规则名, 动作)。
    pub fn evaluate(&self, msg: &Message) -> Option<(&str, Action)> {
        self.rules
            .iter()
            .find(|r| r.matches(msg))
            .map(|r| (r.name.as_str(), r.action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(text: &str) -> Message {
        Message {
            id: 1,
            chat_id: 42,
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
    fn keyword_hide() {
        let mut set = RuleSet {
            rules: vec![Rule {
                name: "ads".into(),
                action: Action::Hide,
                chat_types: vec![],
                chat_ids: vec![],
                keywords: vec!["优惠券".into()],
                patterns: vec![],
                compiled: vec![],
            }],
        };
        set.compile().unwrap();
        assert_eq!(
            set.evaluate(&msg("送你一张优惠券")),
            Some(("ads", Action::Hide))
        );
        assert_eq!(set.evaluate(&msg("正常聊天")), None);
    }

    #[test]
    fn regex_and_scope() {
        let mut set = RuleSet {
            rules: vec![Rule {
                name: "chan-only".into(),
                action: Action::Collapse,
                chat_types: vec![ChatType::Channel],
                chat_ids: vec![],
                keywords: vec![],
                patterns: vec![r"t\.me/".into()],
                compiled: vec![],
            }],
        };
        set.compile().unwrap();
        assert_eq!(set.evaluate(&msg("t.me/spam")), None); // group 不生效
        let mut m = msg("看看 t.me/spam");
        m.chat_type = ChatType::Channel;
        assert_eq!(set.evaluate(&m), Some(("chan-only", Action::Collapse)));
    }
}
