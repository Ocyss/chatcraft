# Lua 脚本（Desktop）

## 自动加载和管理

默认读取 **Telegram 可执行文件所在目录的 `script/`**，不存在时尝试创建。将 `.lua` 文件直接放入此目录；不递归扫描子目录。按路径排序加载，每个脚本使用独立 Lua 环境，单个目录脚本失败不影响其他脚本。

侧边栏的 **Lua 脚本管理** 列出启动时的脚本及已加载/错误状态，显示错误信息，并提供底部 **重启应用** 按钮。状态是加载快照，不是脚本每次执行成功的保证。添加、删除或编辑后均需重启，不支持热加载。

可通过 `CHATCRAFT_SCRIPT_DIR` 更换默认目录；现有 `CHATCRAFT_CONFIG` / `CHATCRAFT_CONFIG_FILE` 的 JSON 中 `script_dir` 优先，设为 `null` 可关闭目录加载。相对路径以工作目录为基准。显式 `lua_script` 保留原有行为，在目录脚本前执行；同一路径不要同时配置两次。显式旧式脚本加载失败仍会导致引擎初始化失败。

## UI 过滤

参考 `examples/lua/ui_filter.lua`。将它复制到 `script/` 并重启，即可隐藏号码确认提醒、Telegram 高级版和我的星星入口。

```lua
local hidden = {
    ["dialogs.confirm_phone"] = true,
    ["settings.premium"] = true,
    ["settings.my_stars"] = true,
}
function filter_ui(item)
    if hidden[item.id] then return "hide" end
end
```

`item.id` 是 Chatcraft 在原生 UI 构建位置定义的稳定语义 ID，不依赖翻译、显示文本、列表位置或服务器临时编号。`item.kind` 区分位置。任何脚本返回 `"hide"` 或 `{action="hide", reason="..."}` 即隐藏；`nil`、其他动作、缺少回调或运行错误不隐藏。每次调用有 Lua 指令数限制。

| ID 前缀 | kind | 支持的后缀 |
| --- | --- | --- |
| `settings.` | `settings` | `my_account`, `notifications`, `privacy`, `chat`, `folders`, `advanced`, `devices`, `power_saving`, `language`, `interface_scale`, `premium`, `my_stars`, `currency`, `business`, `send_gift`, `faq`, `features`, `ask_question` |
| `main_menu.` | `main_menu` | `my_profile`, `bots`, `new_group`, `new_channel`, `contacts`, `calls`, `saved_messages`, `wallet`, `premium`, `my_stars`, `settings` |
| `dialogs.` | `dialog` | `confirm_phone` |

`dialogs.confirm_phone` 对应“此号码是否仍为您的号码”的使用号码提醒，不是登录验证码、安全验证或所有弹窗。过滤只改变 UI，不伪造确认请求、不改变服务器账户状态。设置主列表隐藏的 section 同时从设置搜索索引中排除。脚本管理入口不参与过滤，避免失去管理入口。

这些钩子覆盖上述明确接入的入口，而不是任意第三方 UI 控件；后续新增入口应继续使用固定语义 ID。消息过滤仍使用已有 `filter(msg)`，可与 `filter_ui(item)` 共存。多个消息脚本按加载顺序取首个明确动作，因此仅处理 UI 的脚本不要定义始终返回 `allow` 的消息回调。

只加载可信脚本。Lua 指令限制不等同于完整安全沙箱；脚本在应用进程内运行。
