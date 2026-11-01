# Android CI 测试构建

工作流：`.github/workflows/android.yml`。修改工作流或更新 Android 子模块指针并推送到 `main` 时自动运行，也可以在 GitHub Actions 的 `android` 页面手动运行。

## 必需配置

在主仓库 GitHub Actions Secrets 中设置 `ANDROID_GOOGLE_SERVICES_JSON`，内容为 Firebase 为 Android 应用 `icu.ocyss.chatcraft` 导出的完整 `google-services.json`。配置只写入应用变体的配置目录，不重写 fork 中受版本控制的源码。缺失或包名不匹配时工作流提前失败，不会伪造 Firebase 身份或跳过配置检查。

## 构建与产物

CI 检出主仓库固定的 `clients/android` 提交及其递归子模块，不检出 desktop/iOS。使用 JDK 17、SDK 36、NDK 27.2.12479018、CMake 3.22.1 和上游 Gradle wrapper，执行：

```sh
bash ./gradlew :TMessagesProj_App:assembleAfatRelease -PAPP_PACKAGE=icu.ocyss.chatcraft
```

构建成功后检查 APK 包名和签名，并上传 `chatcraft-android-test-<commit>` artifact，保留 14 天。APK 包名严格为 `icu.ocyss.chatcraft`，不附加 `.beta` 或 `.web`。

## 限制

- 当前使用上游仓库公开的示例 keystore，仅供测试；不要将其作为可信正式发布签名。
- 当前 Android fork 仍保留上游 BuildVars 中的 Telegram API 身份及服务开关。桌面使用的 `CHATCRAFT_API_ID` / `CHATCRAFT_API_HASH` Secrets 尚未接入 Android；正式发布前必须在 fork 中接入自有 API 身份、签名和适用的第三方服务配置，审核 passkeys、商店及更新入口。
- 本工作流仅编译现有 Android fork，不表示 Android 已具备桌面的 Chatcraft Rust 设置功能。
- CI 编译、签名校验不等于设备上的登录、推送和客户端行为验证。
