//! 跨平台共享测试：用 tests/fixtures/messages.json 验证引擎行为，
//! 各平台 FFI 接入后应使用同一文件做一致性校验。

use chatcraft_core::{Action, Engine, EngineConfig, Message};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    message: Message,
    expect: Expect,
}

#[derive(Deserialize)]
struct Expect {
    action: String,
}

#[test]
fn fixtures_match_expectations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let cases: Vec<Case> = serde_json::from_str(
        &std::fs::read_to_string(root.join("tests/fixtures/messages.json")).unwrap(),
    )
    .unwrap();

    let engine = Engine::new(EngineConfig {
        rules: Default::default(),
        lua_script: Some(root.join("scripts/example.lua")),
    })
    .unwrap();

    for case in cases {
        let expected = match case.expect.action.as_str() {
            "allow" => Action::Allow,
            "hide" => Action::Hide,
            "collapse" => Action::Collapse,
            "highlight" => Action::Highlight,
            other => panic!("unknown action {other}"),
        };
        let got = engine.filter(&case.message);
        assert_eq!(
            got.action, expected,
            "message {} mismatched (source: {})",
            case.message.id, got.source
        );
    }
}
