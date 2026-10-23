"""Apply the root-owned Desktop UI overlay before configuring the native build.

The pinned fork supplies the base integration. Keeping pending UI changes as a
tracked patch makes them reproducible without relying on a dirty submodule.
"""

import argparse
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATCH = ROOT / "scripts/desktop-ui.patch"
DESKTOP = ROOT / "clients/desktop"
REQUIRED = {
    "Telegram/SourceFiles/window/window_main_menu.cpp": (
        "void ShowScripts(",
        "Chatcraft::ScriptsStatus()",
        "Chatcraft::ScriptsDirectory()",
        "QDesktopServices::openUrl",
        'u"Lua 脚本管理"_q',
    ),
    "Telegram/SourceFiles/chatcraft/chatcraft_filter.h": (
        "QJsonArray ScriptsStatus()",
        "bool ScriptsDirectoryEnabled()",
    ),
    "Telegram/SourceFiles/chatcraft/chatcraft_filter.cpp": (
        "chatcraft_filter_ui",
        "chatcraft_scripts_status",
        "chatcraft_filter_chat",
        "chatcraft_lifecycle",
        "chatcraft_ui_requests",
        "chatcraft_ui_respond",
        'absoluteFilePath(u"scripts"_q)',
    ),
    "Telegram/SourceFiles/settings/sections/settings_main.cpp": (
        "dialogs.confirm_phone",
        "settings.premium",
        "settings.my_stars",
    ),
    "Telegram/SourceFiles/settings/settings_builder.cpp": ("Chatcraft::IsUiHidden",),
    "Telegram/CMakeLists.txt": (
        "CHATCRAFT_ENABLED=1",
        "TDESKTOP_DISABLE_AUTOUPDATE=1",
    ),
    "Telegram/SourceFiles/dialogs/dialogs_inner_widget.cpp": (
        "Chatcraft::FilterChat",
        "_chatcraftProjection",
    ),
    "Telegram/SourceFiles/dialogs/dialogs_inner_widget.h": ("_chatcraftProjection",),
    "Telegram/SourceFiles/core/application.cpp": (
        'Chatcraft::ShowLifecycleNotifications(u"startup"_q)',
    ),
}


def git(*args):
    return subprocess.run(
        ["git", "-C", str(DESKTOP), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def prepare(check_only=False):
    reverse = git("apply", "--reverse", "--check", str(PATCH))
    if reverse.returncode == 0:
        print("Desktop UI overlay is already applied.")
    elif check_only:
        raise RuntimeError(
            "Desktop UI overlay is missing or differs; run scripts/prepare-desktop.py"
        )
    else:
        check = git("apply", "--check", str(PATCH))
        if check.returncode:
            raise RuntimeError(
                "Cannot apply Desktop UI overlay. Preserve local changes and resolve the conflict:\n"
                + check.stderr
            )
        applied = git("apply", str(PATCH))
        if applied.returncode:
            raise RuntimeError(applied.stderr)
        print("Applied Desktop UI overlay.")
    for relative, markers in REQUIRED.items():
        source = (DESKTOP / relative).read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                raise RuntimeError(
                    f"Desktop feature check failed: {relative}: {marker}"
                )
    print(
        "Verified Desktop script manager, folder opener, UI hooks and Windows FFI definition."
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="Verify without changing sources"
    )
    args = parser.parse_args()
    prepare(args.check)
