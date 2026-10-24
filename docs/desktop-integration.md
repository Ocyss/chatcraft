# Desktop fork integration

## Source ownership

`clients/desktop` pins a commit in `Ocyss/Telegram-Desktop`. The fork contains
`Telegram/SourceFiles/chatcraft/chatcraft_filter.{h,cpp}` and the small native
hooks directly. Pending script-manager and UI-filter integration is shipped as
`scripts/desktop-ui.patch` in the root repository, so a clean CI checkout does
not depend on uncommitted changes inside the submodule.

Before configuring a local Desktop build, run:

```text
python scripts/prepare-desktop.py
```

This applies the overlay once, accepts an already-applied overlay, and stops
on conflicts without overwriting local work. `--check` verifies it without
changing sources. Windows CI runs this after checkout and again before the
native build. If editing the overlay sources, update the tracked patch as well.
Once these changes are published in the Desktop fork, advance the pinned
submodule and remove the corresponding overlay together.

- `History::createItem`: evaluates rules after message construction.
- `HistoryItem`: caches the hidden state.
- `HistoryView::Element::isHidden`: honors filtering and sponsored-message settings.
- `Telegram/CMakeLists.txt`: enables the bridge on Windows.

Other platforms retain the bridge stub until integrated. No generic hooking
framework is needed for these explicit source-level calls.

## CI scope and artifacts

`desktop-win` builds Windows Rust FFI itself and links it into Telegram. It does
not depend on the separate `ffi` workflow. That optional workflow includes
Windows, Linux, macOS, Android and iOS compilation and now runs only through
`workflow_dispatch`, while Desktop testing is the current priority. `core`
continues running workspace tests and clean-checkout overlay regression tests.

The Windows artifact includes `BUILD-INFO.txt` with the root source commit.
Only `scripts/welcome.lua` is enabled by default. `scripts/README.md` is copied
from the root README; `scripts/types/chatcraft.lua` and `.luarc.json` provide
LuaLS completion without executing the type declarations. API documentation is
maintained in `docs/lua/chatcraft.lua`.

The `examples/` directory contains `ad_filter.lua`, `ui_filter.lua` and
`chat_list.lua` with their normal filenames. Copy a script into `scripts/` and
restart to enable it; no extension change is needed.

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
The resulting artifact contains `Telegram.exe`, not an installer. Official
auto-update is disabled and `Updater.exe` is not distributed, to avoid replacing
the custom client. Debug remains the current C++ configuration;
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

## Telegram application identity and local accounts

Register Chatcraft at https://my.telegram.org with the application title
`Chatcraft`. Configure repository Actions secrets `CHATCRAFT_API_ID` and
`CHATCRAFT_API_HASH`; Desktop CI fails early when they are unavailable (including
fork PRs). No upstream test-credential fallback is allowed.

Local builds accept those same environment variables, or explicit CMake options
`-DTDESKTOP_API_ID=<your-id> -DTDESKTOP_API_HASH=<your-hash>`. Explicit options
win; environment values are not copied into CMakeCache.txt. Clear old cached
credentials before switching to environment injection. Never commit credentials,
cache files, build directories or verbose compiler logs: generated compiler
commands and the executable necessarily contain API credentials. A distributed
native client cannot keep its embedded API hash secret from binary inspection.

Native MTProto `initConnection.app_version` is prefixed with `Chatcraft `;
upstream numeric protocol/update versions and real device/OS fields are retained.
The application name shown by other Telegram clients also depends on the server's
application registration for the API ID. Verify it using a newly authorized
session on another device; changing a version field alone cannot guarantee the
server-provided name or rename historical sessions.

`-DCHATCRAFT_MAX_ACCOUNTS=6` configures local account capacity at build time
(default 6, accepted range 1–20). This replaces the local Premium-dependent
3–6 account capacity with the configured capacity for all local accounts. It
neither grants Premium nor removes Telegram login, flood, authorization or other
server-side limits. Restart using the rebuilt executable to apply it. Do not
lower capacity below the number of saved accounts without first removing excess
accounts through the UI; existing storage is not automatically deleted.

Lua is not used for these settings: API credentials and connection identity are
needed during native MTProto/login initialization, before message-filter scripts
are an appropriate configuration layer. Exposing credentials to user scripts
would also unnecessarily broaden access to them.

Native changes live in the Desktop submodule and must be published in the fork
and the root pin advanced before a clean CI checkout can consume them; they are
not part of the UI overlay.

## Configuration

- `CHATCRAFT_SCRIPT_DIR`: override the default `scripts/` next to Telegram.exe.
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
