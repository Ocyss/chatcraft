# Android CI 测试构建

工作流：`.github/workflows/android.yml`。修改工作流或更新 Android 子模块指针并推送到 `main` 时自动运行，也可以在 GitHub Actions 的 `android` 页面手动运行。

## 必需配置

Android CI 复用桌面已有的 `CHATCRAFT_API_ID` 和 `CHATCRAFT_API_HASH` 仓库 Secrets。构建步骤通过同名环境变量传给 Android fork，Gradle 校验后生成 `BuildConfig` 字段供 `BuildVars` 使用；不硬编码凭据、不回退到上游示例值、不在 CI 中重写源码。本地构建也必须设置这两个环境变量。API ID 必须为正 int32，API Hash 必须为 32 位十六进制字符串，缺失或格式错误时直接停止构建。

在主仓库 GitHub Actions Secrets 中设置 `ANDROID_GOOGLE_SERVICES_JSON`，内容为 Firebase 为 Android 应用 `icu.ocyss.chatcraft` 导出的完整 `google-services.json`。配置只写入应用变体的配置目录，不重写 fork 中受版本控制的源码。缺失或包名不匹配时工作流提前失败，不会伪造 Firebase 身份或跳过配置检查。

## 构建与产物

CI 检出主仓库固定的 `clients/android` 提交及其递归子模块，不检出 desktop/iOS。使用 JDK 17、SDK 36、NDK 27.2.12479018、CMake 3.22.1 和上游 Gradle wrapper，执行：

```sh
bash ./gradlew :TMessagesProj_App:assembleAfatRelease -PAPP_PACKAGE=icu.ocyss.chatcraft
```

构建成功后检查 APK 包名和签名，并上传 `chatcraft-android-test-<commit>` artifact，保留 14 天。APK 包名严格为 `icu.ocyss.chatcraft`，不附加 `.beta` 或 `.web`。

## 限制

- 当前使用上游仓库公开的示例 keystore，仅供测试；不要将其作为可信正式发布签名。
- Telegram API 身份已接入自有 Secrets；其他上游服务开关未在本次修改。正式发布前仍需接入正式签名并审核适用的第三方服务配置、passkeys、商店及更新入口。
- 本工作流仅编译现有 Android fork，不表示 Android 已具备桌面的 Chatcraft Rust 设置功能。
- CI 编译、签名校验不等于设备上的登录、推送和客户端行为验证。
