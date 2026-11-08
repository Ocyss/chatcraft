"""Run: python -m unittest discover -s docs -p test_desktop_release.py"""

import ast
import base64
import json
import os
import shutil
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

import desktop_release as release


class ReleaseTests(unittest.TestCase):
    def test_workflow_prepare_arguments_match_pinned_fork(self):
        root = Path(__file__).resolve().parents[1]
        prepare = root / "clients/desktop/Telegram/build/prepare/prepare.py"
        if not prepare.is_file():
            self.skipTest("Desktop submodule required for prepare interface check")
        tree = ast.parse(prepare.read_text(encoding="utf-8"))
        options = next(
            ast.literal_eval(node.value)
            for node in tree.body
            if isinstance(node, ast.Assign)
            and any(
                isinstance(target, ast.Name) and target.id == "optionsList"
                for target in node.targets
            )
        )
        workflow = (root / ".github/workflows/desktop-win.yml").read_text(
            encoding="utf-8"
        )
        prefix = r"%TBUILD%\desktop\Telegram\build\prepare\win.bat "
        calls = [
            line.strip()[len(prefix) :].split()
            for line in workflow.splitlines()
            if line.strip().startswith(prefix)
        ]
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[0], ["silent"])
        self.assertEqual(calls[1], ["skip-release", "silent"])
        for args in calls:
            for arg in args:
                self.assertIn(arg, options + ["silent"])

    def test_release_tag(self):
        for tag in ("desktop-v7.4.1", "desktop-v7.4.1-r1", "desktop-v7.4.1-r12"):
            self.assertEqual(release.release_tag("refs/tags/" + tag, "7.4.1"), tag)
        for tag in (
            "desktop-v7.4.2-r1",
            "desktop-v7.4.1-r0",
            "desktop-v7.4.1-r01",
            "desktop-v7.4.1-r",
            "desktop-v7x4x1-r1",
        ):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.release_tag("refs/tags/" + tag, "7.4.1")
        with self.assertRaises(ValueError):
            release.release_tag("refs/heads/desktop-v7.4.1-r1", "7.4.1")

    def test_native_feed(self):
        self.assertEqual(
            release.feed(7003003, "td-update-win-x64-7003003"),
            {
                "win64": {
                    "stable": {
                        "released": 7003003,
                        "testing": 7003003,
                        "link": "td-update-win-x64-7003003",
                    }
                }
            },
        )

    def test_native_version_and_suffix(self):
        with tempfile.TemporaryDirectory() as directory:
            desktop = Path(directory)
            build = desktop / "Telegram/build"
            build.mkdir(parents=True)
            (build / "version").write_text(
                "AppVersion 7003003\nAppVersionStr 7.3.3\nBetaChannel 0\nAlphaVersion 0\n"
            )
            self.assertEqual(release.version(desktop), (7003003, "7.3.3"))
            source = desktop / "Telegram/lib_base/base/platform/win"
            source.mkdir(parents=True)
            (source / "base_info_win.cpp").write_text(
                "int AutoUpdateVersion() { return 6; }"
            )
            self.assertEqual(release.suffix(desktop), "6")
            (build / "version").write_text(
                "AppVersion 7003003\nAppVersionStr 7.3.3\nBetaChannel 1\n"
            )
            with self.assertRaises(ValueError):
                release.version(desktop)

    def test_preflight_reports_unpinned_submodules(self):
        for status in "-+U":
            with self.subTest(status=status):
                line = status + "a" * 40 + " Telegram/dependency"
                with (
                    patch.dict(
                        os.environ,
                        {
                            "GITHUB_REPOSITORY": "owner/repo",
                            "GITHUB_REF": "refs/tags/desktop-v7.4.1",
                        },
                    ),
                    patch.object(release, "version", return_value=(7004001, "7.4.1")),
                    patch.object(release, "output", return_value=""),
                    patch.object(
                        release.subprocess, "check_output", return_value=line + "\n"
                    ),
                ):
                    with self.assertRaises(ValueError) as error:
                        release.preflight(Path("desktop"), Path("trust"))
                    self.assertIn(line, str(error.exception))

    def test_configure_enables_native_packer(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trust = root / "trust"
            trust.mkdir()
            release.write_json(
                trust / "state.json", {"prefix": "https://example.com/updates"}
            )
            with (
                patch.object(release, "run") as native,
                patch.dict(
                    os.environ, {"CHATCRAFT_API_ID": "123", "CHATCRAFT_API_HASH": "abc"}
                ),
            ):
                release.configure(root / "desktop", trust)
            configure, build = [call.args for call in native.call_args_list]
            self.assertIn("DESKTOP_APP_SPECIAL_TARGET=win64", configure)
            self.assertIn("DESKTOP_APP_DISABLE_AUTOUPDATE=OFF", configure)
            self.assertIn("Packer", build)
            self.assertIn("Updater", build)

    def test_package_payload_and_feed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            desktop, trust, artifacts = (
                root / "desktop",
                root / "trust",
                root / "assets",
            )
            out = desktop / "out/Release"
            out.mkdir(parents=True)
            trust.mkdir()
            release.write_json(
                trust / "state.json",
                {
                    "number": 7003003,
                    "label": "7.3.3",
                    "suffix": "6",
                    "key_id": "release-1",
                },
            )
            for name in ("Telegram.exe", "Updater.exe", "td-update-win-x64-7003003"):
                (out / name).write_bytes(b"test")
            with (
                patch.object(release, "run") as native,
                patch.object(release, "output", return_value="fork-sha"),
                patch.dict(os.environ, {"GITHUB_SHA": "main-sha"}),
            ):
                release.package(desktop, trust, artifacts)
            args = native.call_args.args
            self.assertEqual(args.count("-path"), 2)
            self.assertIn("Telegram.exe", args)
            self.assertIn("Updater.exe", args)
            with zipfile.ZipFile(
                artifacts / "chatcraft-win-x64-7.3.3-portable.zip"
            ) as archive:
                self.assertEqual(
                    set(archive.namelist()),
                    {"Telegram/Telegram.exe", "Telegram/Updater.exe"},
                )
            self.assertEqual(
                json.loads((artifacts / "current6").read_text()),
                release.feed(7003003, "td-update-win-x64-7003003"),
            )
            self.assertEqual(len(list(artifacts.iterdir())), 5)

    def test_publish_uploads_before_exposing_latest(self):
        with tempfile.TemporaryDirectory() as directory:
            artifacts = Path(directory)
            release.write_json(
                artifacts / "state.json", {"tag": "desktop-v7.3.3", "label": "7.3.3"}
            )
            (artifacts / "current6").write_text("{}")
            with (
                patch.object(release, "run") as gh,
                patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/repo"}),
            ):
                release.publish(artifacts, artifacts)
            calls = [call.args for call in gh.call_args_list]
            self.assertEqual([call[2] for call in calls], ["create", "upload", "edit"])
            self.assertIn("--draft", calls[0])
            self.assertIn(artifacts / "current6", calls[1])
            self.assertNotIn(artifacts / "state.json", calls[1])
            self.assertIn("--latest", calls[2])

    @unittest.skipUnless(shutil.which("openssl"), "OpenSSL required")
    def test_trust_and_tampering(self):
        with tempfile.TemporaryDirectory() as directory:
            trust = Path(directory) / "trust"
            release.init_trust(trust)
            release.validate_trust(trust, "release-1")
            with self.assertRaises(FileExistsError):
                release.init_trust(trust)
            manifest = json.loads((trust / "manifest.min.json").read_bytes())
            self.assertEqual(
                len(base64.urlsafe_b64decode(manifest["keys"][0]["x"] + "==")), 32
            )
            (trust / "manifest.min.json").write_bytes(b"{}")
            with self.assertRaises(Exception):
                release.validate_trust(trust, "release-1")


if __name__ == "__main__":
    unittest.main()
