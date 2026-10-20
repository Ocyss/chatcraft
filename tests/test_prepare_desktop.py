"""Exercise the UI overlay against the exact fork revision pinned by Git."""

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "prepare_desktop", ROOT / "scripts/prepare-desktop.py"
)
prepare_desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare_desktop)


class DesktopOverlayTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.desktop = Path(self.temp.name)
        subprocess.run(["git", "init", "--quiet", str(self.desktop)], check=True)
        revision = subprocess.check_output(
            ["git", "--no-pager", "rev-parse", "HEAD:clients/desktop"],
            cwd=ROOT,
            text=True,
        ).strip()
        for relative in prepare_desktop.REQUIRED:
            content = subprocess.check_output(
                ["git", "--no-pager", "show", f"{revision}:{relative}"],
                cwd=ROOT / "clients/desktop",
            )
            target = self.desktop / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)
        self.override = patch.object(prepare_desktop, "DESKTOP", self.desktop)
        self.override.start()
        self.addCleanup(self.override.stop)

    def test_clean_checkout_apply_verify_and_repeat(self):
        with self.assertRaisesRegex(RuntimeError, "overlay is missing"):
            prepare_desktop.prepare(check_only=True)
        prepare_desktop.prepare()
        prepare_desktop.prepare(check_only=True)
        prepare_desktop.prepare()
        menu = (
            self.desktop / "Telegram/SourceFiles/window/window_main_menu.cpp"
        ).read_text(encoding="utf-8")
        self.assertIn("打开脚本文件夹", menu)
        self.assertIn("Lua 脚本管理", menu)
        self.assertIn("重启应用", menu)

    def test_conflicting_local_work_is_not_overwritten(self):
        target = self.desktop / "Telegram/SourceFiles/chatcraft/chatcraft_filter.cpp"
        target.write_text("// locally modified bridge\n", encoding="utf-8")
        before = target.read_bytes()
        with self.assertRaisesRegex(RuntimeError, "Cannot apply Desktop UI overlay"):
            prepare_desktop.prepare()
        self.assertEqual(target.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
