# Chatcraft 核心架构与接入指南

## 布局

```
crates/core   # Rust 内置设置；旧规则引擎和 Lua API 保留兼容
crates/ffi    # C ABI (cdylib + staticlib)，供 Desktop/Android/iOS 调用
examples/lua/ # 历史兼容示例，不随桌面包分发
crates/core/tests/fixtures # 跨平台共享测试用例
clients/      # 官方客户端 fork (git submodule)，原生接入直接提交到 fork
clients/desktop/Telegram/build/chatcraft/ # 桌面构建辅助工具
```

## 设置与源码归属

桌面采用 Rust 内置设置，不由 Lua 管理。原生设置界面、刷新及客户端接入归属 Desktop fork；设置格式、持久化与判断归属 `crates/core`，C ABI 归属 `crates/ffi`。配置位于 Telegram 工作数据目录的 `tdata/chatcraft/settings.json`，同一工作数据目录中的账号共享，不按账号拆分，也不自动云同步。

桌面内置设置包括主列表隐藏已归入任意自定义文件夹的会话、sponsored 广告及稳定界面 ID 可见性。`read_first` 仅在核心中预留，尚未接入桌面排序。旧 Lua API 与广告环境变量保留用于 core 兼容，不作为新桌面设置入口或发布包约定。

客户端改动直接维护在 fork，不使用主仓库 patch、overlay 或构建时重写受版本控制源码。当前 fork 接入仍含本地未提交改动；必须另行授权提交、推送并更新主仓库子模块指针，干净 CI 才能使用该实现。

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

内置设置经独立设置 C ABI 接入，不依赖 Lua 引擎创建。以下消息过滤接口保留作旧 API 兼容，不表示新桌面设置仍通过 Lua 执行：

平台侧把原生消息序列化成 `Message` 的 JSON，调用
`chatcraft_filter_message()`，按返回 JSON 里的 `action`
(`allow|hide|collapse|highlight`) 执行 UI 操作。Lua 和规则不直接接触
任何平台 UI 控件。

`Engine` 非线程安全（Lua 运行时限制），请在同一线程创建与调用。
