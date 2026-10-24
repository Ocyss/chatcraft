# Chatcraft

为 Telegram 客户端提供本地可编程过滤：Rust 核心执行规则与 Lua，C ABI 连接桌面端，支持消息、界面、会话列表过滤及脚本管理。

过滤仅改变本地显示，不删除服务器消息、退出会话或修改服务器文件夹。当前桌面支持消息隐藏、界面隐藏和会话列表隐藏/排序；`collapse`、`highlight` 尚无显示效果。

## 使用

使用启用了 Chatcraft FFI 的桌面构建，从主菜单打开「Lua 脚本管理」，查看实际目录、元数据及加载错误。默认扫描可执行文件旁 `scripts/` 的直接子文件 `.lua`，不递归。修改、添加、删除或停用脚本后，需**完全退出（包括托盘进程）并重启**，不支持热重载。

发布包约定（CI 打包另行维护）：

```text
Telegram.exe
scripts/
  welcome.lua
  README.md
  .luarc.json
  types/
    chatcraft.lua
examples/
  ad_filter.lua
  ui_filter.lua
  chat_list.lua
```

默认只启用 `welcome.lua`：启动不弹窗，首次打开管理器时演示异步确认，不改变过滤。其他示例按需复制到实际脚本目录即可启用，文件均使用正常 `.lua` 后缀；不要把整个示例目录作为默认脚本目录。

## 配置

`CHATCRAFT_SCRIPT_DIR` 可覆盖默认目录，相对路径按进程当前工作目录解析。若 `scripts/` 不存在而旧 `script/` 存在，桌面兼容使用旧目录。

显式配置来源按优先级为：非空 `CHATCRAFT_CONFIG`（JSON 字符串）、读取成功的 `CHATCRAFT_CONFIG_FILE`、`CHATCRAFT_LUA_SCRIPT`（单个脚本路径）。JSON 示例：

```json
{
  "lua_script": null,
  "script_dir": "E:/Chatcraft/scripts"
}
```

原有 `rules` 配置仍受支持。缺少 `script_dir` 时桌面补入默认目录；显式设为 `null` 可关闭目录扫描。只运行一个脚本时，同时指定 `lua_script` 与 `"script_dir": null`；仅设置 `CHATCRAFT_LUA_SCRIPT` 不关闭默认扫描。

显式脚本先加载，再按路径排序加载目录脚本。显式脚本失败会使引擎创建失败；目录中失败脚本被跳过，其他脚本继续。各脚本状态独立，顺序可能影响决策。错误和无效结果按不决策处理，不会撤销其他有效规则。

同一脚本重复定义 `chatcraft.filter_message` 会覆盖旧函数，并非追加。需要组合规则时使用 `chatcraft.register_filter_message(function(msg) ... end)`；界面和会话列表分别使用 `register_filter_ui`、`register_filter_chat_list`。可重复注册，详细聚合规则见 Lua 类型文档。消息回调不匹配时返回 `nil`，不要返回会提前结束匹配的 `allow`。

自有 Telegram API 凭据、Chatcraft 会话身份和本地账号容量配置见 [桌面集成](docs/desktop-integration.md)。这些配置在原生启动/构建阶段生效，不向 Lua 暴露 API 凭据，也不能绕过 Telegram 服务端限制。

## Lua 文档与编辑器

**[docs/lua/chatcraft.lua](docs/lua/chatcraft.lua) 是 Lua API 类型、字段、约束和兼容语义的唯一维护来源**，中文注释可直接通过 LuaLS 悬停和补全查看。示例位于 [examples/lua](examples/lua)。不再在 README 维护另一份 API 表格。

仓库 `.luarc.json` 将 `docs/lua` 加入 LuaLS 类型库，运行时设为 Lua 5.4。类型文件只供编辑器索引，**不要执行或 require**，也不要替换宿主提供的 `chatcraft` 表。

发布时将该类型文件复制到 `scripts/types/chatcraft.lua`，根 README 复制到 `scripts/README.md`；包内 `scripts/.luarc.json` 的 `workspace.library` 改为 `["types"]`，保留 Lua 5.4 和 `workspace.checkThirdParty: false`。`types/` 不参与非递归脚本扫描。

## 构建

需要支持 Rust 2024 edition 的工具链及本地 C/C++ 构建环境（Lua 随 `mlua` 构建）：

```sh
cargo test --workspace
cargo build --release -p chatcraft-ffi
```

桌面构建与接入另见 [桌面集成](docs/desktop-integration.md)，项目设计见 [架构](docs/architecture.md)。桌面源码位于 `clients/desktop` 子模块，Rust 核心与 ABI 位于 `crates/core`、`crates/ffi`；仓库根 `scripts/` 是开发工具目录，不是发布包的运行脚本目录。

## 安全与排错

- **仅运行可信脚本。Lua 环境不是安全沙箱。** 指令限制不等于墙钟超时或内存上限；回调应快速、无阻塞，避免高频过滤路径执行 I/O 或弹窗。
- 不生效时先检查 FFI 是否启用、管理器加载状态、实际目录、`.lua` 后缀及配置覆盖，再完全退出重启并查看日志。
- 脚本 `update_url` 仅展示，不自动联网检查、下载或安装。
- 定制桌面版禁用官方自动更新，避免被官方版本覆盖；暂未配置 Chatcraft 更新源，请手动安装本项目后续版本。
