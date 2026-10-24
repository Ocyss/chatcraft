use chatcraft_core::{Engine, EngineConfig, UiItem};
use std::path::PathBuf;

#[test]
fn registered_handlers_aggregate_and_isolate_failures() {
    use chatcraft_core::{ChatListItem, Message};
    let scripts = Scripts::new();
    scripts.write(
        "a.lua",
        r#"
        chatcraft.register_filter_message(function(m) while true do end end)
        chatcraft.register_filter_message(function(m) return {action='invalid'} end)
        chatcraft.register_filter_message(function(m) return 'allow' end)
        chatcraft.register_filter_message(function(m) error('must short circuit') end)
        function chatcraft.filter_message(m) return 'hide' end
        chatcraft.register_filter_ui(function(i) error('bad') end)
        chatcraft.register_filter_ui(function(i) return 'allow' end)
        chatcraft.register_filter_ui(function(i) return {action='hide',reason='registered'} end)
        function chatcraft.filter_ui(i) return {action='hide',reason='single'} end
        chatcraft.register_filter_chat_list(function(c) while true do end end)
        chatcraft.register_filter_chat_list(function(c) return {priority='bad'} end)
        chatcraft.register_filter_chat_list(function(c) return {priority=7} end)
        chatcraft.register_filter_chat_list(function(c) return {hide=true,priority=99} end)
        function chatcraft.filter_chat_list(c) return {priority=-1} end
    "#,
    );
    let engine = scripts.engine();
    let msg: Message = serde_json::from_value(serde_json::json!({
        "id":1,"chat_id":1,"chat_type":"group","text":"x"
    }))
    .unwrap();
    assert_eq!(
        engine.filter(&msg).action,
        chatcraft_core::rules::Action::Allow
    );
    assert_eq!(
        engine.filter_ui(&item("x")).reason.as_deref(),
        Some("registered")
    );
    let chat: ChatListItem = serde_json::from_value(serde_json::json!({
        "chat_id":1,"chat_type":"group","title":"x","folder_count":0,"folder_id":0
    }))
    .unwrap();
    let outcome = engine.filter_chat(&chat);
    assert!(outcome.hide);
    assert_eq!(outcome.priority, 7);
}

#[test]
fn registration_validation_and_single_callback_compatibility() {
    let scripts = Scripts::new();
    scripts.write(
        "a.lua",
        r#"
        for _, register in ipairs({chatcraft.register_filter_message,
            chatcraft.register_filter_ui, chatcraft.register_filter_chat_list}) do
            assert(not pcall(register))
            assert(not pcall(register, {}))
            assert(not pcall(register, function() end, function() end))
            assert(not pcall(register, chatcraft, function() end))
        end
        function filter_ui(i) return 'hide' end
        function chatcraft.on_ui(i) return 'hide' end
        function chatcraft.filter_ui(i) return 'hide' end
        function chatcraft.filter_ui(i) return nil end
        chatcraft.register_filter_ui(function(i) return 'nonsense' end)
    "#,
    );
    assert!(!scripts.engine().filter_ui(&item("x")).hide);
    scripts.write(
        "a.lua",
        r#"
        chatcraft.register_filter_ui(function(i) error('bad') end)
        function filter_ui(i) return {action='hide',reason='legacy'} end
    "#,
    );
    assert_eq!(
        scripts.engine().filter_ui(&item("x")).reason.as_deref(),
        Some("legacy")
    );
}

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
fn namespaced_chat_metadata_and_lifecycle() {
    use chatcraft_core::{ChatListItem, LifecycleEvent};
    let scripts = Scripts::new();
    scripts.write("a.lua", r#"
        script = {name='Test', version='1', description='demo', author='me', update_url='https://example.org/a.lua'}
        function filter_ui(item) return 'allow' end
        function chatcraft.filter_ui(item) return 'hide' end
        function chatcraft.filter_chat_list(chat)
            assert(chat.folder_count == #chat.folder_ids)
            assert(chatcraft.util == nil)
            assert(chat.title:find('.', 1, true))
            local in_folder = false
            for _, id in ipairs(chat.folder_ids) do
                if id == chat.folder_id then in_folder = true end
            end
            return {hide=not in_folder, priority=(chat.unread or chat.unread_count > 0) and 0 or 1}
        end
        function chatcraft.on_startup(ctx) return {title=ctx.event, text='hello'} end
        function chatcraft.on_manager_open(ctx) return {title=ctx.event, text='manager'} end
    "#);
    let engine = scripts.engine();
    assert_eq!(engine.scripts_status()[0].name, "a.lua");
    assert_eq!(
        engine.scripts_status()[0].metadata.name.as_deref(),
        Some("Test")
    );
    assert!(engine.filter_ui(&item("x")).hide);
    let mut chat: ChatListItem = serde_json::from_value(serde_json::json!({
        "chat_id":1, "chat_type":"group", "title":"a.b", "folder_ids":[2,3],
        "folder_count":2, "folder_id":2
    }))
    .unwrap();
    assert_eq!(engine.filter_chat(&chat).priority, 1);
    assert!(!engine.filter_chat(&chat).hide);
    chat.unread = true;
    assert_eq!(engine.filter_chat(&chat).priority, 0);
    chat.folder_id = 4;
    assert!(engine.filter_chat(&chat).hide);
    assert_eq!(
        engine.lifecycle(LifecycleEvent::Startup)[0].title,
        "startup"
    );
    assert!(engine.lifecycle(LifecycleEvent::Startup).is_empty());
    for _ in 0..2 {
        assert_eq!(
            engine.lifecycle(LifecycleEvent::ManagerOpen)[0].text,
            "manager"
        );
    }
}

#[test]
fn chat_and_lifecycle_errors_fail_open() {
    use chatcraft_core::{ChatListItem, LifecycleEvent};
    let scripts = Scripts::new();
    scripts.write("a.lua", "function chatcraft.on_chat(c) while true do end end; function chatcraft.on_startup(c) error('bad') end");
    scripts.write("b.lua", "function chatcraft.on_chat(c) return {hide=true,priority=9} end; function chatcraft.on_startup(c) return {title='ok',text='ok'} end");
    let engine = scripts.engine();
    let chat: ChatListItem = serde_json::from_value(serde_json::json!({"chat_id":1,"chat_type":"private","title":"x","folder_id":0,"folder_count":0})).unwrap();
    assert!(engine.filter_chat(&chat).hide);
    assert_eq!(engine.filter_chat(&chat).priority, 9);
    assert_eq!(engine.lifecycle(LifecycleEvent::Startup).len(), 1);
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
    assert!(engine.scripts_status()[0]
        .error
        .as_ref()
        .unwrap()
        .contains("instruction limit"));
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
    assert!(Engine::new(EngineConfig {
        lua_script: Some(scripts.0.join("missing.lua")),
        ..Default::default()
    })
    .is_err());
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

#[test]
fn new_callbacks_take_precedence_without_falling_back_on_nil_or_error() {
    use chatcraft_core::{Action, ChatListItem, ChatType, Message};
    let scripts = Scripts::new();
    scripts.write(
        "a.lua",
        r#"
        function filter(m) return 'hide' end
        function filter_ui(i) return 'hide' end
        function chatcraft.on_message(m) return 'hide' end
        function chatcraft.on_ui(i) return 'hide' end
        function chatcraft.on_chat(c) return {hide=true,priority=99} end
        function chatcraft.filter_message(m) return 'highlight' end
        function chatcraft.filter_ui(i)
            if i.id == 'error' then error('new callback error') end
            return nil
        end
        function chatcraft.filter_chat_list(c) return {hide=false,priority=7} end
    "#,
    );
    let engine = scripts.engine();
    let message = Message {
        id: 1,
        chat_id: 1,
        chat_type: ChatType::Private,
        chat_title: None,
        sender_id: None,
        sender_name: None,
        text: "x".into(),
        is_sponsored: false,
        is_forwarded: false,
    };
    assert_eq!(engine.filter(&message).action, Action::Highlight);
    assert!(!engine.filter_ui(&item("nil")).hide);
    assert!(!engine.filter_ui(&item("error")).hide);
    let chat: ChatListItem = serde_json::from_value(serde_json::json!({
        "chat_id":1,"chat_type":"private","title":"x","folder_id":0,"folder_count":0
    }))
    .unwrap();
    assert_eq!(engine.filter_chat(&chat).priority, 7);
    assert!(!engine.filter_chat(&chat).hide);
    scripts.write("a.lua", "function chatcraft.on_ui(i) return 'hide' end");
    assert!(scripts.engine().filter_ui(&item("legacy")).hide);
    scripts.write(
        "a.lua",
        "function chatcraft.on_message(m) return 'collapse' end",
    );
    assert_eq!(scripts.engine().filter(&message).action, Action::Collapse);
}

#[test]
fn async_ui_requests_callbacks_and_limits() {
    use chatcraft_core::{lua::UI_REQUEST_LIMIT, LifecycleEvent};
    let scripts = Scripts::new();
    scripts.write(
        "a.lua",
        r#"
        assert(chatcraft.util == nil)
        chatcraft.utils.alert('loaded', 'ready')
        chatcraft.utils.confirm('question', 'continue?', function(confirmed)
            assert(type(confirmed) == 'boolean')
            chatcraft.utils.alert('answer', tostring(confirmed))
            chatcraft.utils.confirm('next', 'again?', function(value)
                chatcraft.utils.alert('final', tostring(value))
            end)
        end)
        function chatcraft.on_manager_open(ctx)
            for i=1,128 do chatcraft.utils.alert('bounded', 'queue') end
        end
    "#,
    );
    scripts.write(
        "b.lua",
        "chatcraft.utils.alert('discard', 'bad load'); error('load failed')",
    );
    let engine = scripts.engine();
    let requests = engine.ui_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].kind, "alert");
    assert_eq!(requests[1].kind, "confirm");
    assert_ne!(requests[0].id, requests[1].id);
    assert!(engine.ui_requests().is_empty());
    assert!(engine.ui_respond(requests[0].id, true).is_err());
    engine.ui_respond(requests[1].id, true).unwrap();
    assert!(engine.ui_respond(requests[1].id, false).is_err());
    let requests = engine.ui_requests();
    assert_eq!(requests[0].text, "true");
    engine.ui_respond(requests[1].id, false).unwrap();
    assert_eq!(engine.ui_requests()[0].text, "false");
    engine.lifecycle(LifecycleEvent::ManagerOpen);
    assert_eq!(engine.ui_requests().len(), UI_REQUEST_LIMIT);
    engine.lifecycle(LifecycleEvent::ManagerOpen);
    assert_eq!(engine.ui_requests().len(), UI_REQUEST_LIMIT);
}

#[test]
fn pending_confirms_remain_bounded_after_drain_and_callbacks_reset_budget() {
    let scripts = Scripts::new();
    scripts.write(
        "a.lua",
        r#"
        for i=1,128 do
            chatcraft.utils.confirm('q', 'q', function(value)
                if value then while true do end end
                chatcraft.utils.alert('ok', 'recovered')
            end)
        end
        function chatcraft.filter_ui(item)
            chatcraft.utils.confirm('overflow', 'overflow', function(v) end)
        end
    "#,
    );
    let engine = scripts.engine();
    let requests = engine.ui_requests();
    assert_eq!(requests.len(), 128);
    engine.filter_ui(&item("overflow"));
    assert!(engine.ui_requests().is_empty());
    assert!(engine
        .ui_respond(requests[0].id, true)
        .unwrap_err()
        .contains("instruction limit"));
    assert!(engine.ui_respond(requests[0].id, false).is_err());
    engine.ui_respond(requests[1].id, false).unwrap();
    assert_eq!(engine.ui_requests()[0].text, "recovered");
    engine.filter_ui(&item("room"));
    assert_eq!(engine.ui_requests().len(), 1);
}

fn example_engine(name: &str, expected_name: &str, description: &str) -> Engine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/lua")
        .join(name);
    let engine = Engine::new(EngineConfig {
        lua_script: Some(path.clone()),
        ..Default::default()
    })
    .unwrap();
    let statuses = engine.scripts_status();
    assert_eq!(statuses.len(), 1);
    let status = &statuses[0];
    assert_eq!(status.path, path);
    assert_eq!(status.name, name);
    assert!(status.loaded, "{name}: {:?}", status.error);
    assert_eq!(status.error, None);
    assert_eq!(status.metadata.name.as_deref(), Some(expected_name));
    assert_eq!(status.metadata.version.as_deref(), Some("1.0"));
    assert_eq!(status.metadata.description.as_deref(), Some(description));
    assert_eq!(status.metadata.author.as_deref(), Some("Chatcraft"));
    assert_eq!(status.metadata.update_url, None);
    assert!(engine.ui_requests().is_empty());
    engine
}

#[test]
fn example_ad_filter_keywords_and_unmatched_nil() {
    use chatcraft_core::{lua::LuaFilter, Action, ChatType, Message};
    let engine = example_engine(
        "ad_filter.lua",
        "频道推广过滤",
        "隐藏频道推广关键词，演示群转发消息的折叠意图",
    );
    let filter = LuaFilter::from_file(&engine.scripts_status()[0].path).unwrap();
    let mut message = Message {
        id: 1,
        chat_id: 1,
        chat_type: ChatType::Channel,
        chat_title: None,
        sender_id: None,
        sender_name: None,
        text: String::new(),
        is_sponsored: false,
        is_forwarded: false,
    };
    for keyword in ["推广", "代开", "空投"] {
        message.text = format!("频道消息：{keyword}详情");
        let outcome = engine.filter(&message);
        assert_eq!(outcome.action, Action::Hide);
        assert_eq!(outcome.source, "lua");
        assert_eq!(outcome.reason, Some(format!("频道推广关键词：{keyword}")));
    }
    message.text = "普通频道消息".into();
    assert!(matches!(
        filter.call("filter", &message).unwrap(),
        mlua::Value::Nil
    ));
    assert_eq!(engine.filter(&message).source, "default");
    message.chat_type = ChatType::Group;
    message.text = "推广".into();
    assert!(matches!(
        filter.call("filter", &message).unwrap(),
        mlua::Value::Nil
    ));
    message.is_forwarded = true;
    assert_eq!(engine.filter(&message).action, Action::Collapse);
    message.chat_type = ChatType::Private;
    assert!(matches!(
        filter.call("filter", &message).unwrap(),
        mlua::Value::Nil
    ));
}

#[test]
fn example_ui_filter_hides_only_listed_ids() {
    let engine = example_engine(
        "ui_filter.lua",
        "界面精简",
        "隐藏宿主已接入的手机号确认、会员与星星入口",
    );
    for id in [
        "dialogs.confirm_phone",
        "settings.premium",
        "settings.my_stars",
        "main_menu.premium",
        "main_menu.my_stars",
    ] {
        let outcome = engine.filter_ui(&item(id));
        assert!(outcome.hide, "{id}");
        assert_eq!(outcome.source, "lua");
        assert_eq!(outcome.reason.as_deref(), Some("界面精简示例"));
    }
    let outcome = engine.filter_ui(&item("settings.notifications"));
    assert!(!outcome.hide);
    assert_eq!(outcome.source, "default");
}

#[test]
fn example_chat_list_main_folder_hiding_read_priority_and_archive() {
    use chatcraft_core::ChatListItem;
    let engine = example_engine(
        "chat_list.lua",
        "会话列表隐藏与已读优先",
        "主列表隐藏属于多个自定义文件夹的会话，其余已读优先",
    );
    let mut chat: ChatListItem = serde_json::from_value(serde_json::json!({
        "chat_id":1, "chat_type":"group", "title":"example", "folder_id":0,
        "folder_ids":[2,3], "folder_count":2, "unread":false, "unread_count":0
    }))
    .unwrap();
    for folder_id in [0, -1, 2] {
        chat.folder_id = folder_id;
        for folder_count in [0, 1, 2, 3] {
            chat.folder_ids = (2..2 + folder_count as i64).collect();
            chat.folder_count = folder_count;
            for (unread, unread_count, priority) in
                [(false, 0, 1), (true, 0, 0), (false, 3, 0), (true, 3, 0)]
            {
                chat.unread = unread;
                chat.unread_count = unread_count;
                let outcome = engine.filter_chat(&chat);
                assert_eq!(
                    outcome.hide,
                    folder_id == 0 && folder_count > 1,
                    "folder_id={folder_id}, folder_count={folder_count}"
                );
                assert_eq!(
                    outcome.priority, priority,
                    "unread={unread}, unread_count={unread_count}"
                );
            }
        }
    }
}

#[test]
fn example_welcome_manager_confirmation_then_alert() {
    use chatcraft_core::LifecycleEvent;
    for confirmed in [true, false] {
        let engine = example_engine(
            "welcome.lua",
            "Chatcraft 使用说明",
            "打开管理器时演示异步确认与提示，不改变过滤结果",
        );
        assert!(engine.lifecycle(LifecycleEvent::Startup).is_empty());
        assert!(engine.ui_requests().is_empty());
        assert!(engine.lifecycle(LifecycleEvent::ManagerOpen).is_empty());
        let requests = engine.ui_requests();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.kind, "confirm");
        assert_eq!(request.title, "Chatcraft");
        assert_eq!(request.text, "是否查看脚本使用说明？");
        assert!(engine.ui_requests().is_empty());
        engine.ui_respond(request.id, confirmed).unwrap();
        let alerts = engine.ui_requests();
        if confirmed {
            assert_eq!(alerts.len(), 1);
            assert_eq!(alerts[0].kind, "alert");
            assert_ne!(alerts[0].id, request.id);
            assert_eq!(alerts[0].title, "Chatcraft Lua 脚本使用说明");
            assert!(alerts[0].text.contains("仅安装可信脚本"));
            assert!(alerts[0]
                .text
                .contains("ad_filter.lua、ui_filter.lua、chat_list.lua"));
        } else {
            assert!(alerts.is_empty());
        }
        assert!(engine.ui_respond(request.id, confirmed).is_err());
        assert!(engine.lifecycle(LifecycleEvent::ManagerOpen).is_empty());
        assert!(engine.ui_requests().is_empty());
    }
}

#[test]
fn lua_type_document_has_no_runtime_side_effects() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/lua/chatcraft.lua");

    let source = std::fs::read_to_string(path).unwrap();
    let lua = mlua::Lua::new();
    lua.load("chatcraft = {marker = true, utils = {marker = true}}; original = chatcraft; original_utils = chatcraft.utils").exec().unwrap();
    lua.load("before = {}; for k,v in pairs(_G) do before[k] = v end")
        .exec()
        .unwrap();
    lua.load(&source).exec().unwrap();
    lua.load("assert(chatcraft == original and chatcraft.utils == original_utils and chatcraft.marker and chatcraft.utils.marker)").exec().unwrap();
    lua.load("for k,v in pairs(_G) do assert(before[k] == v, 'global changed: '..k) end; for k,v in pairs(before) do assert(_G[k] == v) end; for k in pairs(chatcraft) do assert(k == 'marker' or k == 'utils') end; for k in pairs(chatcraft.utils) do assert(k == 'marker') end").exec().unwrap();
}
