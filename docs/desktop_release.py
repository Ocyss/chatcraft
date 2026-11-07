"""Windows native release tooling. Python stdlib + OpenSSL; never rewrites sources."""

import argparse
import base64
import json
import os
import re
import shutil
import subprocess
import time
import zipfile
from pathlib import Path


def run(*args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def output(*args):
    return subprocess.check_output([str(a) for a in args], text=True).strip()


def write_json(path, value):
    path.write_text(json.dumps(value, separators=(",", ":")), encoding="utf-8")


def public_der(private):
    return subprocess.check_output(
        ["openssl", "pkey", "-in", str(private), "-pubout", "-outform", "DER"]
    )


def init_trust(directory):
    directory = directory.resolve()
    repo = Path(__file__).resolve().parents[1]
    if directory == repo or repo in directory.parents:
        raise ValueError("Trust directory must be outside the repository")
    directory.mkdir(mode=0o700, parents=True, exist_ok=False)
    for name in ("root", "package"):
        run(
            "openssl",
            "genpkey",
            "-algorithm",
            "ED25519",
            "-out",
            directory / f"{name}-private.pem",
        )
        (directory / f"{name}-private.pem").chmod(0o600)
    run(
        "openssl",
        "pkey",
        "-in",
        directory / "root-private.pem",
        "-pubout",
        "-out",
        directory / "root-public.pem",
    )
    coordinate = (
        base64.urlsafe_b64encode(public_der(directory / "package-private.pem")[-32:])
        .decode()
        .rstrip("=")
    )
    write_json(
        directory / "manifest.min.json",
        {
            "format": 1,
            "manifest_version": 1,
            "issued": int(time.time()),
            "keys": [{"id": "release-1", "alg": "Ed25519", "x": coordinate}],
            "channels": {"stable": [["release-1"]]},
            "revoked": [],
        },
    )
    run(
        "openssl",
        "pkeyutl",
        "-sign",
        "-rawin",
        "-inkey",
        directory / "root-private.pem",
        "-in",
        directory / "manifest.min.json",
        "-out",
        directory / "manifest.sig",
    )
    print(
        "Trust initialized outside repository. Keep root-private.pem offline; package key ID: release-1"
    )


def version(desktop):
    text = (desktop / "Telegram/build/version").read_text()
    fields = dict(re.findall(r"^(\w+)\s+(\S+)", text, re.M))
    if int(fields.get("BetaChannel", "0")) or int(fields.get("AlphaVersion", "0")):
        raise ValueError("Only stable native versions may be released")
    return int(fields["AppVersion"]), fields["AppVersionStr"]


def feed(version_number, package):
    return {
        "win64": {
            "stable": {
                "released": version_number,
                "testing": version_number,
                "link": package,
            }
        }
    }


def suffix(desktop):
    text = (
        desktop / "Telegram/lib_base/base/platform/win/base_info_win.cpp"
    ).read_text()
    match = re.search(r"int AutoUpdateVersion\(\)\s*\{\s*return (\d+);", text)
    if not match:
        raise ValueError("Review changed native AutoUpdateVersion implementation")
    value = int(match[1])
    return str(value) if value > 1 else ""


def validate_trust(directory, key_id):
    run(
        "openssl",
        "pkeyutl",
        "-verify",
        "-pubin",
        "-inkey",
        directory / "root-public.pem",
        "-rawin",
        "-in",
        directory / "manifest.min.json",
        "-sigfile",
        directory / "manifest.sig",
    )
    manifest = json.loads((directory / "manifest.min.json").read_bytes())
    now = int(time.time())
    if (
        manifest.get("format") != 1
        or not 1 <= manifest.get("manifest_version", 0) <= 0xFFFFFFFF
    ):
        raise ValueError("Invalid manifest format/version")
    if manifest.get("expires", 0) and manifest["expires"] <= now:
        raise ValueError("Expired manifest")
    keys = {key["id"]: key for key in manifest["keys"]}
    key = keys[key_id]
    if key_id in manifest["revoked"] or (
        key.get("expires", 0) and key["expires"] <= now
    ):
        raise ValueError("Package key revoked or expired")
    groups = manifest["channels"]["stable"]
    if not groups or any(key_id not in group for group in groups):
        raise ValueError("Single CI signer must satisfy every stable signature group")
    der = public_der(directory / "package-private.pem")
    if key["alg"] != "Ed25519" or der != bytes.fromhex(
        "302a300506032b6570032100"
    ) + base64.urlsafe_b64decode(key["x"] + "=="):
        raise ValueError("Package private key does not match manifest Ed25519 key")


def preflight(desktop, trust):
    repo = os.environ["GITHUB_REPOSITORY"]
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("Invalid repository")
    number, label = version(desktop)
    tag = "desktop-v" + label
    if os.environ["GITHUB_REF"] != "refs/tags/" + tag:
        raise ValueError(
            f"Release must run on existing tag {tag}, matching native version"
        )
    if output("git", "status", "--porcelain", "--untracked-files=no"):
        raise ValueError("Release requires a clean, pinned source checkout")
    if output("git", "-C", desktop, "status", "--porcelain", "--untracked-files=no"):
        raise ValueError("Desktop source must be clean")
    lines = subprocess.check_output(
        ["git", "-C", str(desktop), "submodule", "status", "--recursive"], text=True
    ).splitlines()
    if any(line[0] in "-+U" for line in lines):
        raise ValueError("Desktop nested submodules must match recorded commits")
    options = (desktop / "Telegram/cmake/generate_update_keys.cmake").read_text()
    if "CHATCRAFT_UPDATE_MANIFEST_SIG" not in options:
        raise ValueError(
            "Pinned fork lacks the native ChatCraft updater trust interface"
        )
    # Fail before the hours-long dependency build, including on API/auth failures.
    releases = json.loads(
        output(
            "gh", "api", "--paginate", "--slurp", f"repos/{repo}/releases?per_page=100"
        )
    )
    for page in releases:
        for release in page:
            if release["tag_name"] == tag:
                raise ValueError(
                    "Release already exists; never overwrite signed release assets"
                )
            if not release["draft"] and not release["prerelease"]:
                for asset in release["assets"]:
                    match = re.fullmatch(r"td-update-win-x64-(\d+)", asset["name"])
                    if match and int(match[1]) >= number:
                        raise ValueError(
                            "Native AppVersion must increase beyond all published stable updates"
                        )
    trust.mkdir(mode=0o700, parents=True, exist_ok=False)
    try:
        values = {
            "root-public.pem": os.environ["CHATCRAFT_UPDATE_ROOT_PEM"].encode(),
            "manifest.min.json": os.environ["CHATCRAFT_UPDATE_MANIFEST"].encode(),
            "manifest.sig": base64.b64decode(
                os.environ["CHATCRAFT_UPDATE_MANIFEST_SIG_BASE64"], validate=True
            ),
            "package-private.pem": os.environ["CHATCRAFT_UPDATE_PACKAGE_KEY"].encode(),
        }
        for name, data in values.items():
            (trust / name).write_bytes(data)
            (trust / name).chmod(0o600)
        key_id = os.environ["CHATCRAFT_UPDATE_KEY_ID"]
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,64}", key_id):
            raise ValueError("Invalid package key ID")
        validate_trust(trust, key_id)
        state = {
            "number": number,
            "label": label,
            "tag": tag,
            "suffix": suffix(desktop),
            "key_id": key_id,
            "prefix": f"https://github.com/{repo}/releases/latest/download",
        }
        write_json(trust / "state.json", state)
    except BaseException:
        shutil.rmtree(trust)
        raise


def configure(desktop, trust):
    state = json.loads((trust / "state.json").read_text())
    args = [
        "cmd",
        "/c",
        "configure.bat",
        "x64",
        "-D",
        "TDESKTOP_API_TEST=OFF",
        "-D",
        "CHATCRAFT_API_ID=" + os.environ["CHATCRAFT_API_ID"],
        "-D",
        "CHATCRAFT_API_HASH=" + os.environ["CHATCRAFT_API_HASH"],
        "-D",
        "CHATCRAFT_MAX_ACCOUNTS=6",
        "-D",
        "CMAKE_CONFIGURATION_TYPES=Release",
        "-D",
        "DESKTOP_APP_SPECIAL_TARGET=win64",
        "-D",
        "DESKTOP_APP_DISABLE_AUTOUPDATE=OFF",
        "-D",
        "TDESKTOP_UPDATE_CHANNEL=stable",
        "-D",
        "DESKTOP_APP_DISABLE_CRASH_REPORTS=OFF",
        "-D",
        "CHATCRAFT_UPDATE_PREFIX=" + state["prefix"],
    ]
    for option, name in (
        ("ROOT_PEM", "root-public.pem"),
        ("MANIFEST", "manifest.min.json"),
        ("MANIFEST_SIG", "manifest.sig"),
    ):
        args += ["-D", "CHATCRAFT_UPDATE_" + option + "=" + (trust / name).as_posix()]
    run(*args, cwd=desktop / "Telegram")
    run(
        "cmake",
        "--build",
        desktop / "out",
        "--config",
        "Release",
        "--target",
        "Telegram",
        "Updater",
        "Packer",
        "--parallel",
    )


def package(desktop, trust, artifacts):
    state = json.loads((trust / "state.json").read_text())
    out = desktop / "out/Release"
    name = f"td-update-win-x64-{state['number']}"
    run(
        out / "Packer.exe",
        "-version",
        state["number"],
        "-path",
        "Telegram.exe",
        "-path",
        "Updater.exe",
        "-target",
        "win64",
        "-channel",
        "stable",
        "-keys-loc",
        trust,
        "-local-key",
        trust / "package-private.pem",
        "-local-key-id",
        state["key_id"],
        cwd=out,
    )
    artifacts.mkdir(parents=True, exist_ok=False)
    shutil.copy2(out / name, artifacts / name)
    with zipfile.ZipFile(
        artifacts / f"chatcraft-win-x64-{state['label']}-portable.zip",
        "w",
        zipfile.ZIP_DEFLATED,
    ) as archive:
        for binary in ("Telegram.exe", "Updater.exe"):
            archive.write(out / binary, "Telegram/" + binary)
    write_json(artifacts / ("current" + state["suffix"]), feed(state["number"], name))
    (artifacts / "BUILD-INFO.txt").write_text(
        f"Main: {os.environ['GITHUB_SHA']}\nDesktop: {output('git', '-C', desktop, 'rev-parse', 'HEAD')}\n"
        f"Native AppVersion: {state['number']}\nChannel: stable\n",
        encoding="utf-8",
    )
    # Public build metadata only; never copy the trust directory into artifacts.
    write_json(artifacts / "state.json", state)


def publish(trust, artifacts):
    state = json.loads((trust / "state.json").read_text())
    repo = os.environ["GITHUB_REPOSITORY"]
    run(
        "gh",
        "release",
        "create",
        state["tag"],
        "--repo",
        repo,
        "--verify-tag",
        "--draft",
        "--title",
        "ChatCraft Desktop " + state["label"],
        "--generate-notes",
    )
    run(
        "gh",
        "release",
        "upload",
        state["tag"],
        *sorted(path for path in artifacts.iterdir() if path.name != "state.json"),
        "--repo",
        repo,
    )
    # Only expose latest after every package and discovery asset has uploaded.
    run(
        "gh",
        "release",
        "edit",
        state["tag"],
        "--repo",
        repo,
        "--draft=false",
        "--latest",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=("init-trust", "preflight", "configure", "package", "publish"),
    )
    parser.add_argument("--trust", type=Path, required=True)
    parser.add_argument("--desktop", type=Path, default=Path("clients/desktop"))
    parser.add_argument("--artifacts", type=Path, default=Path("release-artifacts"))
    args = parser.parse_args()
    desktop, trust, artifacts = (
        args.desktop.resolve(),
        args.trust.resolve(),
        args.artifacts.resolve(),
    )
    if args.command == "init-trust":
        init_trust(trust)
    elif args.command == "preflight":
        preflight(desktop, trust)
    elif args.command == "configure":
        configure(desktop, trust)
    elif args.command == "package":
        package(desktop, trust, artifacts)
    else:
        publish(trust, artifacts)


if __name__ == "__main__":
    main()
