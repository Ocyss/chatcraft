# Desktop fork integration

## Source ownership

`clients/desktop` pins a commit in `Ocyss/Telegram-Desktop`. The fork contains
`Telegram/SourceFiles/chatcraft/chatcraft_filter.{h,cpp}` and the small native
hooks directly. There is no build-time patch application.

- `History::createItem`: evaluates rules after message construction.
- `HistoryItem`: caches the hidden state.
- `HistoryView::Element::isHidden`: honors filtering and sponsored-message settings.
- `Telegram/CMakeLists.txt`: enables the bridge on Windows.

Other platforms retain the bridge stub until integrated. No generic hooking
framework is needed for these explicit source-level calls.

## Unified Rust library

Upstream prepare builds tlottie and wallet-engine through a generated
`tdesktop_rust` crate. After prepare, run:

```text
python scripts/build-desktop-rust.py --libraries <Libraries/win64>
```

Run this from an x64 MSVC Native Tools environment. The script uses upstream's
private Rust installation and toolchain, adds the local `chatcraft-ffi` crate,
and rebuilds the existing `tdesktop_rust.lib` with the same Win7 target and
static CRT options. The C++ build links only that combined Rust static library.
There is no separate Chatcraft static library or runtime DLL.

The CI workflow performs this automatically. Upstream may temporarily build a
DLL for wallet-engine binding generation; that is not a distributed dependency.
The resulting artifact contains `Telegram.exe` and the separate upstream
`Updater.exe`, not an installer. Debug remains the current C++ configuration;
this migration does not promise official-release binary size.

## Versioning

Keep upstream `AppVersion` and `AppVersionStr` unchanged. About displays the
upstream version followed by `+chatcraft.r1`. Override the integration revision
with `-DCHATCRAFT_REVISION=2` when configuring. This suffix is display-only and
does not modify Telegram protocol, update, or data-format version numbers.

## Updating upstream

1. In the Desktop fork, fetch the official upstream remote.
2. Merge the chosen upstream commit into the integration branch; resolve conflicts.
3. Push the fork commit.
4. Update the root repository's desktop submodule pointer and run CI.
5. Verify historical/new messages, edits, albums and sponsored messages manually.

Do not automatically advance the root submodule to an untested fork head.
Textual merge success is not evidence of behavioral compatibility.

## Configuration

- `CHATCRAFT_LUA_SCRIPT`: Lua filter path.
- `CHATCRAFT_CONFIG_FILE`: EngineConfig JSON file.
- `CHATCRAFT_CONFIG`: inline EngineConfig JSON (highest precedence).
- `CHATCRAFT_DISABLE_ADS` / `CHATCRAFT_ENABLE_ADS`: sponsored-message settings.

Restart the process after changing configuration. Run creation and filtering on
the same thread; the Lua engine is not Send.

## Current limitations

Hidden messages still contribute to unread counts and chat-list previews.
Message edits and rule changes do not currently re-evaluate existing items.
Only `hide` has native rendering support; `collapse` and `highlight` are reserved.
Sponsored messages are hidden at the view layer, not removed from the protocol.
