use chatcraft_core::{Engine, EngineConfig, UiItem};
use std::path::PathBuf;

struct Scripts(PathBuf);
impl Scripts {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chatcraft-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).unwrap();
    }
    fn engine(&self) -> Engine {
        Engine::new(EngineConfig {
            script_dir: Some(self.0.clone()),
            ..Default::default()
        })
        .unwrap()
    }
}
impl Drop for Scripts {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn item(id: &str) -> UiItem {
    UiItem {
        id: id.into(),
        kind: "button".into(),
    }
}

#[test]
fn sorted_isolated_loads_and_serializable_statuses() {
    let scripts = Scripts::new();
    scripts.write("z.lua", "function filter_ui(item) if leaked then error('shared state') end return {action='hide', reason='last'} end");
    scripts.write(
        "a.lua",
        "leaked = true; function filter_ui(item) return 'allow' end",
    );
    scripts.write("b.lua", "this is invalid Lua!");
    scripts.write("ignored.txt", "error('must not load')");
    std::fs::create_dir(scripts.0.join("directory.lua")).unwrap();
    let engine = scripts.engine();
    let statuses = engine.scripts_status();
    assert_eq!(
        statuses.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["a.lua", "b.lua", "z.lua"]
    );
    assert!(statuses[0].loaded);
    assert!(!statuses[1].loaded);
    assert!(statuses[1].error.is_some());
    assert_eq!(statuses[2].path, scripts.0.join("z.lua"));
    let json = serde_json::to_value(statuses).unwrap();
    assert_eq!(json[1]["loaded"], false);
    assert!(json[1]["error"].is_string());
    assert_eq!(
        engine.filter_ui(&item("target")).reason.as_deref(),
        Some("last")
    );
}

#[test]
fn stable_ids_kind_and_first_hide_wins() {
    let scripts = Scripts::new();
    scripts.write("a.lua", "function filter_ui(item) if item.id == 'toolbar.ads' and item.kind == 'button' then return {action='hide', reason='first'} end end");
    scripts.write("z.lua", "function filter_ui(item) if item.id == 'toolbar.ads' then return {action='hide', reason='second'} end end");
    let engine = scripts.engine();
    assert_eq!(
        engine.filter_ui(&item("toolbar.ads")).reason.as_deref(),
        Some("first")
    );
    assert!(!engine.filter_ui(&item("toolbar.settings")).hide);
    assert!(
        !engine
            .filter_ui(&UiItem {
                id: "other".into(),
                kind: "label".into()
            })
            .hide
    );
}

#[test]
fn load_and_callback_limits_fail_open_and_reset() {
    let scripts = Scripts::new();
    scripts.write("a.lua", "while true do end");
    scripts.write("b.lua", "function filter_ui(item) if item.id == 'loop' then while true do end end if item.id == 'error' then error('bad callback') end return 'hide' end");
    let engine = scripts.engine();
    assert!(!engine.scripts_status()[0].loaded);
    assert!(
        engine.scripts_status()[0]
            .error
            .as_ref()
            .unwrap()
            .contains("instruction limit")
    );
    assert!(!engine.filter_ui(&item("loop")).hide);
    assert!(!engine.filter_ui(&item("error")).hide);
    assert!(engine.filter_ui(&item("ok")).hide);
}

#[test]
fn missing_directory_reported_and_legacy_behavior_preserved() {
    let scripts = Scripts::new();
    let engine = Engine::new(EngineConfig {
        script_dir: Some(scripts.0.join("missing")),
        ..Default::default()
    })
    .unwrap();
    assert!(!engine.scripts_status()[0].loaded);
    assert!(!engine.filter_ui(&item("anything")).hide);
    assert!(
        Engine::new(EngineConfig {
            lua_script: Some(scripts.0.join("missing.lua")),
            ..Default::default()
        })
        .is_err()
    );
    scripts.write(
        "legacy.txt",
        "function filter_ui(item) return {action='hide', reason='legacy'} end",
    );
    scripts.write(
        "a.lua",
        "function filter_ui(item) return {action='hide', reason='directory'} end",
    );
    let engine = Engine::new(EngineConfig {
        lua_script: Some(scripts.0.join("legacy.txt")),
        script_dir: Some(scripts.0.clone()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(engine.scripts_status().len(), 2);
    assert_eq!(
        engine.filter_ui(&item("anything")).reason.as_deref(),
        Some("legacy")
    );
}

#[test]
fn directory_message_filters_continue_after_errors_in_order() {
    use chatcraft_core::{Action, ChatType, Message};
    let scripts = Scripts::new();
    scripts.write("a.lua", "function filter(msg) error('bad script') end");
    scripts.write("b.lua", "function filter(msg) if msg.text == 'hide' then return {action='hide', reason='directory'} end return 'allow' end");
    scripts.write("z.lua", "function filter(msg) return 'highlight' end");
    let engine = scripts.engine();
    let mut message = Message {
        id: 1,
        chat_id: 1,
        chat_type: ChatType::Group,
        chat_title: None,
        sender_id: None,
        sender_name: None,
        text: "hide".into(),
        is_sponsored: false,
        is_forwarded: false,
    };
    let outcome = engine.filter(&message);
    assert_eq!(outcome.action, Action::Hide);
    assert_eq!(outcome.reason.as_deref(), Some("directory"));
    message.text = "allow".into();
    assert_eq!(engine.filter(&message).action, Action::Allow);
}

#[test]
fn missing_and_invalid_callbacks_allow() {
    let scripts = Scripts::new();
    scripts.write("a.lua", "function filter(msg) return 'hide' end");
    scripts.write(
        "b.lua",
        "function filter_ui(item) return {action='nonsense'} end",
    );
    scripts.write("c.lua", "filter_ui = 42");
    scripts.write("d.lua", "function filter_ui(item) return 'highlight' end");
    assert!(!scripts.engine().filter_ui(&item("anything")).hide);
    let config: EngineConfig = serde_json::from_str("{}").unwrap();
    assert!(config.script_dir.is_none());
}
