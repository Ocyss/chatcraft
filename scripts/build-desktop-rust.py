"""Merge ChatCraft into prepare.py's generated Windows Rust static library."""

import argparse
import os
import re
import shutil
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--libraries", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    prepare = root / "clients/desktop/Telegram/build/prepare/prepare.py"
    source = prepare.read_text(encoding="utf-8")
    toolchain = re.search(r"^rustToolchain = '([^']+)'$", source, re.MULTILINE)
    if not toolchain:
        raise RuntimeError("Cannot determine upstream Rust toolchain")
    if os.name != "nt" or os.environ.get("Platform", "").lower() != "x64":
        raise RuntimeError("Run from an x64 MSVC Native Tools environment on Windows")
    libraries = args.libraries.resolve()
    crate = libraries / "tdesktop_rust"
    third_party = root / "clients/ThirdParty"
    cargo_home = third_party / "rust/cargo"
    cargo = cargo_home / "bin/cargo.exe"
    env = os.environ.copy()
    # Match prepare.py, including its private rustup installation and Win7 std build.
    env.update(
        RUSTUP_HOME=str(third_party / "rust/rustup"),
        CARGO_HOME=str(cargo_home),
        RUSTUP_TOOLCHAIN=toolchain.group(1),
        RUSTC_BOOTSTRAP="1",
        PATH=str(cargo_home / "bin") + os.pathsep + env["PATH"],
    )
    # Do not let runner-level flags override the upstream target/CRT configuration.
    for key in (
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_BUILD_TARGET",
        "CARGO_TARGET_DIR",
    ):
        env.pop(key, None)

    def run(*command):
        subprocess.run([str(cargo), *command], cwd=crate, env=env, check=True)

    if not (crate / "Cargo.toml").is_file():
        raise RuntimeError(f"Missing generated crate: {crate}; run prepare first")
    run("add", "--path", str(root / "crates/ffi"))
    lib = crate / "src/lib.rs"
    text = lib.read_text(encoding="utf-8")
    export = "pub use ::chatcraft_ffi;"
    if export not in text:
        lib.write_text(text.rstrip() + "\n" + export + "\n", encoding="utf-8")
    target = "x86_64-win7-windows-msvc"
    run(
        "rustc",
        "--lib",
        "--release",
        "--crate-type",
        "staticlib",
        "-Z",
        "build-std=std,panic_abort",
        "--target",
        target,
        "--config",
        "profile.release.opt-level='z'",
        "--config",
        "profile.release.lto='thin'",
        "--config",
        "profile.release.codegen-units=1",
        "--config",
        "profile.release.panic='unwind'",
        "--config",
        "profile.release.package.tlottie.opt-level=3",
        "--config",
        f"target.{target}.rustflags=['-C','target-feature=+crt-static']",
        "--",
        "--print",
        "native-static-libs",
    )
    built = crate / f"target/{target}/release/tdesktop_rust.lib"
    symbols = subprocess.run(
        ["dumpbin", "/nologo", "/linkermember:1", str(built)],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    for symbol in (
        "chatcraft_engine_new",
        "chatcraft_filter_message",
        "chatcraft_ads_disabled",
    ):
        if symbol not in symbols:
            raise RuntimeError(f"Unified Rust library is missing {symbol}")
    output = crate / "out/lib/tdesktop_rust.lib"
    output.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(built, output)
    print(f"Staged unified static library: {output}")


if __name__ == "__main__":
    main()
