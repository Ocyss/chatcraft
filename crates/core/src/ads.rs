//! 广告开关：通过环境变量禁用官方广告（sponsored messages）。
//!
//! 优先级：`CHATCRAFT_DISABLE_ADS` > `CHATCRAFT_ENABLE_ADS`（显式启用可覆盖禁用）。
//! 取值 "1"/"true"/"yes"（大小写不敏感）视为真。

use std::sync::atomic::{AtomicBool, Ordering};

static OVERRIDE: AtomicBool = AtomicBool::new(false);
static OVERRIDE_SET: AtomicBool = AtomicBool::new(false);

fn env_truthy(name: &str) -> Option<bool> {
    std::env::var(name).ok().map(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// 官方广告是否应被禁用。
pub fn ads_disabled() -> bool {
    if OVERRIDE_SET.load(Ordering::Relaxed) {
        return OVERRIDE.load(Ordering::Relaxed);
    }
    if env_truthy("CHATCRAFT_DISABLE_ADS").unwrap_or(false) {
        return true;
    }
    if env_truthy("CHATCRAFT_ENABLE_ADS").unwrap_or(false) {
        return false;
    }
    // 魔改版默认即禁用广告。
    true
}

/// 平台侧消息流中调用：true 表示该 sponsored 消息应被丢弃。
pub fn should_drop_sponsored(is_sponsored: bool) -> bool {
    is_sponsored && ads_disabled()
}

/// 供测试或 FFI 显式覆盖（如用户在设置页切换）。
pub fn set_ads_disabled_override(disabled: bool) {
    OVERRIDE.store(disabled, Ordering::Relaxed);
    OVERRIDE_SET.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_sponsored_by_default() {
        assert!(should_drop_sponsored(true));
        assert!(!should_drop_sponsored(false));
    }
}
