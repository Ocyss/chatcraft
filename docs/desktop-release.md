# Windows native releases and self-update

`desktop-win.yml` keeps ordinary main/PR/manual builds in Debug with auto-update
**disabled** and no Updater distributed. Release is explicitly opt-in: push a
`desktop-vVERSION-rN` tag, or dispatch the workflow on that existing tag with
`release: true`. There is no Scoop dependency, second version counter, source
patching, or build-time source overlay.

## Source and version prerequisites

First review, commit and push native changes in the desktop fork, then explicitly
update the main repository's `clients/desktop` gitlink to that published commit.
Agents are authorized to complete these commits, pushes and submodule pointer
updates for the current task after checking the worktree and remotes; the release
helper itself never performs these Git operations. CI checks out the recorded gitlink and recursively
initializes the fork dependencies. A local dirty fork is not reproducible in CI.
Preflight rejects a pinned fork missing the external trust interface.

The native version comes only from `Telegram/build/version`: currently
`AppVersion=7004001`, `AppVersionStr=7.4.1`. Release tags use
`desktop-v7.4.1-r1`, `desktop-v7.4.1-r2`, and so on: `7.4.1` is the native base
version and `rN` is the ChatCraft release revision. Agents query remote tags and
select the next unused revision, including failed tags when determining the
maximum. A new base version starts at `r1`. Existing tags (including legacy tags
without a revision) are never moved or reused. The helper accepts legacy tags
for compatibility. The revision suffix does not change native `AppVersion` or
bypass the strictly increasing update-version requirement. Stable updates require a **strictly larger native AppVersion**;
rebuilding 7004001 never updates another 7004001 client. Increment the native base
version in the fork for the next release, preserving its version-field conventions.
Do not introduce an independent ChatCraft version. Beta/alpha are not supported
by this workflow. Existing release tags/assets are never overwritten; preflight
also rejects versions not greater than previously published stable packages.

## One-time trust setup (local, never in CI)

Requires Python 3 and OpenSSL with Ed25519 support. Create the trust directory
**outside every Git checkout**, preferably on an encrypted offline volume:

```powershell
python docs/desktop_release.py init-trust --trust E:/secure/chatcraft-release
```

The command refuses an existing directory and any directory within this checkout.
It generates two Ed25519 keys: an offline root and a package signer. The compact
manifest contains the package public key and a stable AND-of-ORs policy
`[["release-1"]]`. The root signs the exact manifest bytes using OpenSSL
`pkeyutl -sign -rawin`. The root private key is **never** needed in GitHub.
The initial manifest/key have no expiry to avoid stranding offline clients;
operators should select a rotation/expiry policy before deployment. If adding
expiry, re-sign and distribute the new manifest well before it expires.

Back up root and package private keys securely. Never add keys, generated trust
files, CI trust directories or private-key logs to Git, caches or artifacts.
On Windows, additionally restrict the trust directory ACL to the operator:
Python's POSIX permission bits alone do not establish a private Windows ACL.
The helper uses only stdlib and invokes installed OpenSSL; it installs nothing
and makes no network calls during initialization.

## GitHub configuration

Create environment **`desktop-release`**, require trusted reviewer approval and
restrict deployment tags to `desktop-v*`. Protect matching tags against arbitrary
creation/movement. Review the main workflow, helpers and pinned fork before
approving: approved build code executes with release credentials. Ordinary builds
use `desktop-ci` and must not receive signing secrets. Put these secrets only in
`desktop-release`:

| Secret                                 | Exact value                                             |
| -------------------------------------- | ------------------------------------------------------- |
| `CHATCRAFT_UPDATE_ROOT_PEM`            | Contents of `root-public.pem`                           |
| `CHATCRAFT_UPDATE_MANIFEST`            | Exact contents of `manifest.min.json`, no added newline |
| `CHATCRAFT_UPDATE_MANIFEST_SIG_BASE64` | Single-line standard base64 of raw `manifest.sig`       |
| `CHATCRAFT_UPDATE_PACKAGE_KEY`         | Contents of `package-private.pem`                       |
| `CHATCRAFT_UPDATE_KEY_ID`              | `release-1`                                             |

For the signature, PowerShell can print the required value locally:

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes('E:/secure/chatcraft-release/manifest.sig'))
```

Set the existing `CHATCRAFT_API_ID` and `CHATCRAFT_API_HASH` secrets for builds as
before. Only the separate publication job has GitHub `contents: write`; the
build/signing job has `contents: read`. Both release jobs use the protected
environment (GitHub may require approval for both). No PAT or root-private-key
secret is required. Release credentials are not passed to the normal Debug path.
GitHub CLI, Python, OpenSSL and the native Windows toolchain must be installed on
the runner (the workflow uses `windows-latest`).

Preflight runs before dependency compilation, validates the root signature,
manifest policy, signer/public-key match, expiry and revocation, source pins,
tag/version and existing releases. Trust inputs are written under `RUNNER_TEMP`,
not in the fork, and removed in an `always()` cleanup step. No keys are generated
in CI. A cancelled/aborted runner may skip cleanup; use ephemeral hosted runners,
not a persistent shared runner.

## Build and publish

Release dependencies are prepared with `win.bat silent`, retaining the upstream
default Debug and Release dependency builds. This fork supports `skip-release`
for Debug-only CI, but does not support `skip-debug`. The native build selects
Release and explicitly builds **Telegram, Updater and Packer**. CMake receives:

- `DESKTOP_APP_SPECIAL_TARGET=win64` (enables the native Packer target)
- `DESKTOP_APP_DISABLE_AUTOUPDATE=OFF`
- `TDESKTOP_UPDATE_CHANNEL=stable`
- `CHATCRAFT_UPDATE_PREFIX=https://github.com/OWNER/REPO/releases/latest/download`
- absolute external `CHATCRAFT_UPDATE_ROOT_PEM`, `CHATCRAFT_UPDATE_MANIFEST` and
  `CHATCRAFT_UPDATE_MANIFEST_SIG` paths

The native fork disables official MTP discovery when this prefix is configured.
Packer is invoked with `-version AppVersion -path Telegram.exe -path Updater.exe
-target win64 -channel stable -keys-loc TRUST -local-key PACKAGE_PRIVATE_KEY
-local-key-id release-1`. This follows upstream `Telegram/build/build.bat`'s
Windows x64 payload: both executables, no Packer or private keys. Packer verifies
its trust manifest and signs the native v2 envelope; it is not a zip update.
ChatCraft compiles Packer with the upstream `PACKER_DISABLE_PRIVATE` switch so
v2 signing does not require Telegram's private legacy RSA/alpha headers.

Published assets:

- `td-update-win-x64-7003003`: signed native update package (example version).
- `chatcraft-win-x64-7.3.3-portable.zip`: `Telegram/Telegram.exe` and
  `Telegram/Updater.exe` for initial/manual installation.
- **`current6`**: native discovery JSON for `win64.stable`, with `released`,
  `testing` and a relative `link` to the package in the same latest release.
- `BUILD-INFO.txt`: main/fork commits, native version and channel.

The suffix is read from the pinned nested `Telegram/lib_base` Windows
`Platform::AutoUpdateVersion()` implementation, which currently returns **6**.
It is not `current` or `current2`. A changed implementation that the helper cannot
parse requires explicit review rather than guessing. Only the matching native
feed is emitted. GitHub public release downloads must be anonymously accessible;
private repositories are unsuitable for this unauthenticated client feed.

The build uploads a public CI artifact bundle containing the release assets and
`state.json` (public version/feed/key-ID metadata only, no trust files). A separate
least-privilege publication job downloads that bundle, excludes `state.json` from
GitHub Release assets, creates a draft, uploads all assets, then publishes it as latest.
Partial upload failure leaves a draft and does not intentionally change latest;
inspect/delete that incomplete draft before retrying. Do not publish it manually
without checking every asset. Avoid unrelated releases becoming repository
`latest`, since that would break the fixed feed URL. Release runs share a concurrency group to avoid workflow publication races.
GitHub does not guarantee queue ordering; if a newer version runs first, a later
older run is rejected. Preflight cannot prevent another actor publishing outside
this workflow between its check and publication.

The package signature authenticates the update payload; discovery JSON and zip
are not separately signed. Downloading a portable zip is manual bootstrap trust.
These keys are **not** Windows Authenticode certificates; Windows may show
SmartScreen warnings. No installer/code-signing service is added here.

## Rotation and verification

Keep the root stable. For signer rotation, add the new public key to a higher
`manifest_version`, retain keys needed by existing clients, root-sign the exact
new manifest and update the environment inputs. A single CI signer must satisfy
every stable policy group; multi-signer policies need an explicitly reviewed
signing workflow. Never replace an existing root casually: installed clients
pin it. Use the native manifest adoption/revocation rules and test compatibility
before revoking old keys.

Run local helper tests:

```powershell
python -m unittest discover -s docs -p test_desktop_release.py
```

A successful CI build is not a runtime update test. Before broadly distributing,
install an older release with the same root, publish a higher native AppVersion,
check for updates, verify download completion, restart through the native updater
and confirm the running version changed while account data remains intact. Also
test invalid signatures (must reject), unchanged version (must not offer an
update), interrupted downloads and unwritable installation directories. Do not
use ordinary updater-disabled Debug artifacts for this test.
