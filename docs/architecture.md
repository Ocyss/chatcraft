# Chatcraft 核心架构与接入指南

## 布局

```
crates/core   # Rust 内置 settings
crates/ffi    # 设置 C ABI (cdylib + staticlib)，供 Desktop/Android/iOS 调用
clients/      # 官方客户端 fork (git submodule)，原生接入直接提交到 fork
clients/desktop/Telegram/build/chatcraft/ # 桌面构建辅助工具
```

## 设置与源码归属

桌面采用 Rust 内置设置，不由 Lua 管理。原生设置界面、刷新及客户端接入归属 Desktop fork；设置格式、持久化与判断归属 `crates/core`，C ABI 归属 `crates/ffi`。配置位于 Telegram 工作数据目录的 `tdata/chatcraft/settings.json`，同一工作数据目录中的账号共享，不按账号拆分，也不自动云同步。

桌面内置设置包括主列表隐藏已归入任意自定义文件夹的会话、sponsored 广告及稳定界面 ID 可见性。`read_first` 仅在核心中预留，尚未接入桌面排序。Lua、关键词规则引擎、旧 FFI 与 fixtures 已删除，core 仅保留 settings。

客户端改动直接维护在 fork，不使用主仓库 patch、overlay 或构建时重写受版本控制源码。当前 fork 接入仍含本地未提交、未推送改动，主仓库子模块指针尚未更新，干净 CI 尚不能复现该实现；提交、推送及更新指针必须另行授权。

## 构建

独立核心库（不直接作为 Desktop 最终链接库）：

```sh
cargo build -p chatcraft-ffi --release
```

Windows Desktop 使用上游 `tdesktop_rust` 统一静态构建，见
[Desktop 接入指南](desktop-integration.md)。无需额外 Chatcraft DLL。

Android（需安装 NDK 和对应 target，或使用 cargo-ndk）：

```sh
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o clients/android/chatcraft/jniLibs \
    build -p chatcraft-ffi --release
```

iOS：

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo build -p chatcraft-ffi --release --target aarch64-apple-ios
# 再用 xcodebuild -create-xcframework 打包
```

## 平台接入约定

内置设置经设置 C ABI 读取、判断与保存，原生界面负责交互与刷新；广告及已分组会话隐藏仍由原生接入执行。

Desktop 在构建时生成 UI ID 目录，目前共 166 项，替代原有 22 项白名单。settings builder 的稳定字面量 ID 与 main-menu 已接入，显隐列表提供独立固定高度滚动区域与搜索。计算型 ID 和 Qt `objectName` 尚未通用覆盖，目录不代表所有控件均可隐藏；未知项目默认显示。
