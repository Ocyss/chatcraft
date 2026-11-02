# Android CI 测试构建

工作流：`.github/workflows/android.yml`。修改工作流或更新 Android 子模块指针并推送到 `main` 时自动运行，也可以在 GitHub Actions 的 `android` 页面手动运行。

## 必需配置

Android CI 复用桌面已有的 `CHATCRAFT_API_ID` 和 `CHATCRAFT_API_HASH` 仓库 Secrets。构建步骤通过同名环境变量传给 Android fork，Gradle 校验后生成 `BuildConfig` 字段供 `BuildVars` 使用；不硬编码凭据、不回退到上游示例值、不在 CI 中重写源码。本地构建也必须设置这两个环境变量。API ID 必须为正 int32，API Hash 必须为 32 位十六进制字符串，缺失或格式错误时直接停止构建。

在主仓库 GitHub Actions Secrets 中设置 `ANDROID_GOOGLE_SERVICES_JSON`，内容为 Firebase 为 Android 应用 `icu.ocyss.chatcraft` 导出的完整 `google-services.json`。配置只写入应用变体的配置目录，不重写 fork 中受版本控制的源码。缺失或包名不匹配时工作流提前失败，不会伪造 Firebase 身份或跳过配置检查。

## 构建与产物

CI 检出主仓库固定的 `clients/android` 提交及其递归子模块，不检出 desktop/iOS。使用 JDK 17、SDK 36、NDK 27.2.12479018、CMake 3.22.1、stable Rust 和上游 Gradle wrapper。Rust 需安装 `aarch64-linux-android`、`armv7-linux-androideabi`、`i686-linux-android`、`x86_64-linux-android` targets；CI 先运行 core/FFI host 测试，再执行：

```sh
bash ./gradlew :TMessagesProj_App:assembleAfatRelease -PAPP_PACKAGE=icu.ocyss.chatcraft
```

构建成功后检查 APK 包名和签名，并上传 `chatcraft-android-test-<commit>` artifact，保留 14 天。APK 包名严格为 `icu.ocyss.chatcraft`，不附加 `.beta` 或 `.web`。

## Rust core 与原生设置

Android fork 的 `TMessagesProj/jni/chatcraft.cmake` 在现有 CMake 构建内按 ABI 执行 `cargo build --locked --release -p chatcraft-ffi`，使用 NDK API 23 linker，将正常生成的静态库与薄 JNI 编入 `tmessages.49`，没有源码 overlay 或构建时重写。Cargo 产物放在各 ABI 的 CMake binary directory；需从本主仓库布局构建 fork，以找到 `crates/` 和锁文件。无需 cargo-ndk 或新的 JNI Rust 依赖。为避免与上游预编译 tlottie 的 `rust_eh_personality` 强定义冲突，使用同一 NDK 的 `llvm-objcopy` 将 Chatcraft 归档内该符号的定义与引用重命名为 `chatcraft_rust_eh_personality`，输出独立派生归档供链接；不修改 Cargo 原始归档、上游预编译库或受版本控制的源码，不放宽链接器重复定义检查，也不改变 panic 策略。

通过原生「隐私与安全」页右上角 **Chatcraft** 进入设置。Rust settings 保存在应用私有 `files/chatcraft/settings.json`，所有 Telegram 账号共用，不随选中账号切换或账号退出而删除；清除应用数据/卸载仍会删除。JNI 通过 Java 串行化对进程级 handle 的访问，真实 UTF-8 转换避免 JSON 中补充平面字符损坏。设置编辑保留未知字段，读写错误在页面显示，不把保存失败当作成功。

- 「隐藏已分组会话」只处理默认主列表的展示副本，按当前账号任意非默认自定义 folder 的实际 `includesDialog` 判定；秘密聊天与原生 folder 构建一致，以 `encryptedChat.user_id` 判定成员身份，同时保留原 `Dialog` 传入过滤器。不修改底层会话、folder、archive 或选择器数据。保存后广播各已登录账号的列表刷新通知。开启该设置且当前页面展示主列表过滤副本时，禁止置顶拖动（movement flags、长按拖动、onMove 和 adapter 直接移动均防护），避免副本顺序与共享 `pinnedNum`、服务端顺序不一致；关闭设置恢复原拖动行为，custom folder/archive/选择器不受此限制，不重构排序。
- 「隐藏 sponsored」阻止频道/bot sponsored 获取与插入，以及搜索 sponsored peers 请求与结果接入。已有打开聊天中的广告不原地删除，切换后重新进入聊天；不是对所有广告或第三方控件的覆盖承诺。
- `TMessagesProj/chatcraft.gradle` 扫描 `ui/` 下实际 `ChatcraftCore.visible("android.…") ? rowCount++ : -1` 原生行构建点，生成 build 目录内的 `ChatcraftUiIds.java`，不是 `R.id` 资源目录。覆盖范围明确限定为 `PrivacySettingsActivity` 的 30 个独立 leaf 设置行：两步验证、自动删除、密码锁、passkeys、登录邮箱、黑名单、设备会话；电话号码、最后上线、头像、转发、通话、语音、非联系人消息、生日、礼物、简介、音乐、群组邀请；新聊天归档、删除账号、Passport、清除支付信息、bot 生物识别、网站会话；删除联系人、同步联系人、联系人推荐、秘密聊天地图与网页预览。原有功能/版本/账号可用性条件保持不变，目录列出可控制项不代表当前账号都能显示。section/header/detail/shadow 和 Chatcraft 入口不隐藏；不是其他设置页面或全客户端控件覆盖。稳定 ID 不依赖行号/资源编号；重复 ID 或空扫描使生成失败。
- 设置页可搜索 key/稳定 ID 并切换显隐。返回隐私页时重新构建行索引、刷新 adapter；Chatcraft 入口自身不可隐藏，可恢复所有已接入的行。不支持的控件不会被列为可过滤控件；未识别的持久化 ID 保留。

可独立运行 host ABI 冒烟测试：`cargo build --locked -p chatcraft-ffi` 后执行 `python clients/android/Tools/test_chatcraft_core.py target/debug/libchatcraft_ffi.so`（Windows 改用 `target/debug/chatcraft_ffi.dll`）。该测试先检查 30 个实际 leaf gate、拖动入口/adapter 防护与秘密聊天成员 ID 转换的源码契约，再覆盖真实 C ABI 的 UTF-8、未知字段保留、重开持久化、显隐、sponsored、main/folder/archive 判定与失败保存；源码契约检查不执行 Java，不是 JNI 编译或设备行为测试。

本地修改 fork 源码不等于固定 gitlink 的 CI 已可复现。集成时需先发布 Android fork 提交，再更新主仓库子模块指针，CI 只构建该固定版本。

## 本地多账号容量

Java 账号槽位与现有 native 的 5 个连接实例对齐，最多同时登录 5 个账号。添加入口不再按会员状态扣减槽位，也不再展示添加账号的会员推销；满额仍拒绝添加。不修改服务器返回的 Premium 状态，不解锁服务端会员权益。第 5 个账号的登录、切换、推送、退出及重启恢复仍需设备验证。

## 限制

- 当前使用上游仓库公开的示例 keystore，仅供测试；不要将其作为可信正式发布签名。
- Telegram API 身份已接入自有 Secrets；其他上游服务开关未在本次修改。passkeys 已关闭；正式发布前仍需接入正式签名并审核适用的第三方服务配置、商店及更新入口。
- Android 只接入上述设置与明确的界面过滤点，没有全客户端界面覆盖，也没有接入 core 预留的 read-first 排序；不改动其他权益逻辑。
- CI 编译、签名校验不等于设备上的登录、推送和客户端行为验证。
