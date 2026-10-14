use serde::{Deserialize, Serialize};

/// 会话类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatType {
    Private,
    Group,
    Supergroup,
    Channel,
}

/// 统一的跨平台消息模型。平台侧负责把原生消息转换成该结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub chat_id: i64,
    pub chat_type: ChatType,
    pub chat_title: Option<String>,
    pub sender_id: Option<i64>,
    pub sender_name: Option<String>,
    pub text: String,
    /// 是否为官方广告消息（Telegram sponsored message）。
    #[serde(default)]
    pub is_sponsored: bool,
    /// 是否来自频道的自动转发等，预留给规则使用。
    #[serde(default)]
    pub is_forwarded: bool,
}

impl Message {
    pub fn is_channel_message(&self) -> bool {
        matches!(self.chat_type, ChatType::Channel | ChatType::Supergroup)
    }
}
