# Chatcraft 核心架构与接入指南

## 布局

```
crates/core   # 纯 Rust 跨平台核心：规则引擎、Lua 脚本、广告开关
crates/ffi    # C ABI (cdylib + staticlib)，供 Desktop/Android/iOS 调用
scripts/      # 开发与 CI 构建工具（Python 等）
examples/lua/ # 客户端运行时 Lua 过滤示例，不自动加载
tests/fixtures # 跨平台共享测试用例
clients/      # 官方客户端 fork (git submodule)，原生接入直接提交到 fork
```

## 环境变量

| 变量                    | 取值       | 作用                                  |
| ----------------------- | ---------- | ------------------------------------- |
| `CHATCRAFT_DISABLE_ADS` | 1/true/yes | 禁用官方 sponsored 广告（默认即禁用） |
| `CHATCRAFT_ENABLE_ADS`  | 1/true/yes | 显式恢复官方广告                      |

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

平台侧把原生消息序列化成 `Message` 的 JSON，调用
`chatcraft_filter_message()`，按返回 JSON 里的 `action`
(`allow|hide|collapse|highlight`) 执行 UI 操作。Lua 和规则不直接接触
任何平台 UI 控件。

`Engine` 非线程安全（Lua 运行时限制），请在同一线程创建与调用。
