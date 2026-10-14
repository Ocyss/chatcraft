# Desktop 接入补丁说明

## 结构

```
patches/desktop/0001-chatcraft-filter.patch   # 对 tdesktop 子模块的全部修改
clients/desktop/                              # fork 子模块，保持干净，构建前打补丁
```

应用 / 还原：

```sh
scripts/apply-patches.sh desktop        # 或 Windows 下 apply-patches.bat
git -C clients/desktop checkout -- .    # 还原（注意会丢掉未生成的补丁修改）
```

修改子模块后重新生成补丁：

```sh
git -C clients/desktop add -N Telegram/SourceFiles/chatcraft
git -C clients/desktop diff > patches/desktop/0001-chatcraft-filter.patch
```

## 补丁内容

| 位置 | 改动 |
|---|---|
| `Telegram/SourceFiles/chatcraft/chatcraft_filter.{h,cpp}` | 新增 FFI 桥：把 HistoryItem 序列化成 JSON 调 `chatcraft_filter_message()`，出错安全降级为 Allow |
| `history/history.cpp` `createItem` | 消息创建时评估过滤，结果缓存到 item（覆盖历史加载 + 实时收信） |
| `history/history_item.h` | 新增 `_filteredOut` 标记 |
| `history/view/history_view_element.cpp` `isHidden()` | 被过滤消息或被禁广告不渲染（复用现有 hidden 机制，布局/合并/跳转自动适配） |
| `Telegram/CMakeLists.txt` | 链接 `chatcraft/chatcraft_ffi.dll`，POST_BUILD 拷贝 dll 到 exe 旁 |

## 期望的 FFI 库布局（构建前准备好）

```
clients/desktop/chatcraft/
├── chatcraft_ffi.dll
└── lib/chatcraft_ffi.lib
```

CI 里由 `desktop-win.yml` 自动完成；本地：

```powershell
cargo build -p chatcraft-ffi --release
mkdir clients\desktop\chatcraft\lib -Force
copy target\release\chatcraft_ffi.dll clients\desktop\chatcraft\
copy target\release\chatcraft_ffi.dll.lib clients\desktop\chatcraft\lib\chatcraft_ffi.lib
scripts\apply-patches.bat
# 然后按 docs/building-win.md 构建（先跑 Telegram\build\prepare\win.bat）
```

找不到 dll 时 CMake 会以警告降级为无过滤构建（`CHATCRAFT_ENABLED` 未定义时桥接层为 stub）。

## 运行时配置

| 环境变量 | 作用 |
|---|---|
| `CHATCRAFT_LUA_SCRIPT` | Lua 过滤脚本路径（最简用法） |
| `CHATCRAFT_CONFIG_FILE` | EngineConfig JSON 文件（规则 + Lua） |
| `CHATCRAFT_CONFIG` | 直接传 JSON 字符串 |
| `CHATCRAFT_DISABLE_ADS` / `CHATCRAFT_ENABLE_ADS` | 广告开关 |

## 已知边界

- 被隐藏消息仍计入未读数、会话列表最后一条消息（`setChatListMessage` 未跳过）——需要时再加补丁
- 消息被编辑不会重新评估过滤（编辑走 `updateEditedMessage`，item 不重建）
- `collapse` / `highlight` 动作目前 FFI 已返回但桌面端未实现渲染（hide 已生效）
- sponsored 消息不走 `createItem`，由 `isHidden()` 里的 `AdsDisabled()` 直接拦截
