# Chatcraft

Telegram 客户端的本地设置与过滤扩展。当前桌面方案采用 **Rust 内置设置 + 原生设置界面**。Lua、关键词规则引擎及旧 FFI 已删除，core 仅保留 settings。

## 设置与配置

桌面设置由 fork 原生界面接入 Rust，通过 C ABI 读取、判断并保存。桌面设置支持隐藏已归入任意自定义文件夹的主列表会话、隐藏 sponsored 广告及按稳定 ID 隐藏界面入口。`read_first` 仅在核心中预留，尚未接入桌面排序，也不提供桌面开关。它们仅改变本地显示，不删除服务器消息、退出会话、授予 Premium 或绕过服务端限制。

配置保存在 Telegram 工作数据目录下的 `tdata/chatcraft/settings.json`。同一安装使用同一工作数据目录的账号共享这份配置；不同工作数据目录分别保存，配置不会经 Telegram 自动同步。迁移安装时需连同相应数据目录迁移；不要把个人配置打入公共发布包。保存失败时应检查原生界面的错误提示及数据目录权限。

当前配置格式版本为 `1`，默认不隐藏已分组会话、隐藏 sponsored 广告，且不隐藏其他界面入口。UI 显隐列表使用桌面构建时生成的 UI ID 目录，目前共 166 项，替代原有 22 项白名单。settings builder 的稳定字面量 ID 与 main-menu 已接入；列表提供独立固定高度滚动区域与搜索。计算型 ID 和 Qt `objectName` 尚未通用覆盖，不能据此认为所有控件都可隐藏；未知项目默认显示。隐藏设置页入口不等于禁止访问，该入口仍可能出现在原生设置搜索中。启用会话过滤时暂不支持在主列表拖动置顶顺序，可在关闭过滤后调整。设置由 Rust 持久化，原生界面负责交互与刷新。客户端实际行为仍需完成 fork 接入后的编译与手工验证，不能以核心测试代替。

Telegram API 凭据、连接身份、本地账号容量与构建说明见 [桌面集成](docs/desktop-integration.md)。这些配置由原生构建与接入管理。

## 源码与构建

- `crates/core`：Rust 内置 settings。
- `crates/ffi`：设置专用跨语言 C ABI。
- `clients/desktop`、`clients/android`、`clients/ios`：各客户端 fork 子模块，原生实现直接维护在 fork。
- `clients/desktop/Telegram/build/chatcraft/build-desktop-rust.py`：桌面统一 Rust 静态库构建辅助工具。

不再使用主仓库 patch、overlay 或构建前源码重写维护客户端实现。Windows CI 检出固定子模块提交，沿用上游 prepare 工具链，并将 Chatcraft 编入既有 `tdesktop_rust.lib`；产物不需要独立 Chatcraft DLL。

```sh
cargo test --workspace
cargo build --release -p chatcraft-ffi
```

**当前 fork 存在本地未提交接入改动，迁入的构建脚本也尚未提交、推送，主仓库子模块指针未推进。因此干净 CI 尚不能复现这份本地集成。** 需要另行授权发布 fork 提交并更新主仓库指针后再验证 CI。

项目设计见 [架构](docs/architecture.md)。定制桌面版禁用官方自动更新，不分发 `Updater.exe`；暂未配置 Chatcraft 更新源，请手动安装后续版本。
