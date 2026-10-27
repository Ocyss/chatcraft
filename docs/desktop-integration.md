# Desktop fork integration

## Source ownership

`clients/desktop` pins a commit in `Ocyss/Telegram-Desktop`. All native UI,
settings bridges and integration hooks belong directly in that fork. Rust
settings and persistence belong in `crates/core`, with the C ABI in `crates/ffi`.
Desktop uses Rust built-in settings. Lua, the keyword-rule engine and the old
FFI have been removed; core retains only settings.

The root-owned patch, prepare overlay and overlay tests have been removed.
Do not apply an overlay or rewrite tracked client sources before building.
The build helper lives in the fork's existing `Telegram/build/chatcraft/`
directory; it only extends upstream's generated Rust build crate.

The current local fork integration and moved build helper are not committed or
pushed, and the root submodule pin has not been advanced. A clean CI checkout
cannot reproduce this local integration yet. Publishing the fork commit and
updating the root pin require separate authorization; local source checks or
build success do not remove that requirement.

- Native chat-list integration hides grouped chats from the main list.
- `HistoryView::Element::isHidden`: honors sponsored-message visibility settings.
- `Telegram/CMakeLists.txt`: enables the bridge on Windows.

Other platforms retain the bridge stub until integrated. No generic hooking
framework is needed for these explicit source-level calls.

## CI scope and artifacts

`desktop-win` builds Windows Rust FFI itself and links it into Telegram. It does
not depend on the separate `ffi` workflow. That optional workflow includes
Windows, Linux, macOS, Android and iOS compilation and now runs only through
`workflow_dispatch`, while Desktop testing is the current priority. `core`
continues running workspace tests; the old fixtures have been removed.
It no longer fetches Desktop or runs overlay tests.

The Windows artifact includes `Telegram.exe`, the root README and
`BUILD-INFO.txt` recording root and Desktop commits. It does not package Lua
runtime scripts, examples, LuaLS configuration or types.

## Unified Rust library

Upstream prepare builds tlottie and wallet-engine through a generated
`tdesktop_rust` crate. After prepare, run:

```text
python clients/desktop/Telegram/build/chatcraft/build-desktop-rust.py --libraries <Libraries/win64>
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

Native changes live in the Desktop submodule and must be published in the fork
and the root pin advanced before a clean CI checkout can consume them; they are
not supplied by any root-owned overlay.

## Configuration

Native settings use the Rust settings ABI. The file is
`tdata/chatcraft/settings.json` beneath
Telegram's working data directory. Accounts in the same installation using that
data directory share it; separate working data directories have separate files.
It is not automatically synchronized to another device. Preserve the applicable
data directory when moving an installation and do not distribute personal
settings in public artifacts.

Version 1 stores `hide_grouped_chats`, `read_first`, `hide_sponsored` and
`hidden_ui_ids`. Defaults are false, false, true and an empty set respectively.
The native UI saves through Rust and notifies native subscribers to refresh;
actual client behavior needs compilation and manual validation of the completed
fork integration. Unknown JSON fields are retained; unsupported versions are
not overwritten. Save failures must be reported rather than silently accepted.

## UI ID catalog

Desktop generates the UI ID catalog at build time. It currently contains 166
entries, replacing the previous 22-entry whitelist. Stable literal IDs in the
settings builder and main-menu entries are integrated. The visibility list has
its own fixed-height scroll area and search.

Computed IDs and Qt `objectName` values are not generically covered. Catalog
size is not a promise that every control can be hidden; unknown entries remain
visible by default.

## Current limitations

Grouped-chat filtering affects only the main-list presentation, not folder
membership, unread counts, archive or search results. Pinned dragging is disabled
while this presentation is active; turn filtering off to reorder pinned chats.
`read_first` is reserved in core and has no Desktop ordering hook or UI switch.
The UI catalog does not cover arbitrary controls. Hidden settings-page buttons can
still appear in native settings search; visibility is not access control.
Sponsored messages are hidden at the view layer, not removed from the protocol.
