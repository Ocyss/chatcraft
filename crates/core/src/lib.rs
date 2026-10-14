//! Chatcraft 跨平台核心：消息过滤、规则引擎、Lua 脚本、广告开关。
//!
//! 桌面端 / Android / iOS 共用同一份行为逻辑，平台侧只做 FFI 接入。

pub mod ads;
pub mod engine;
pub mod lua;
pub mod message;
pub mod rules;

pub use engine::{Engine, EngineConfig, FilterOutcome};
pub use message::{ChatType, Message};
pub use rules::{Action, Rule, RuleSet};
