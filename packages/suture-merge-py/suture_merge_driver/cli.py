#!/usr/bin/env python3
"""suture-merge-driver CLI — locates the suture binary and delegates merge-file."""

import os
import platform
import shutil
import subprocess
import sys


def detect_platform() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()
    arch_map = {"x86_64": "x64", "amd64": "x64", "aarch64": "arm64", "arm64": "arm64"}
    os_map = {"linux": "linux", "darwin": "darwin", "windows": "win32"}
    arch = arch_map.get(machine)
    os_name = os_map.get(system)
    if not arch or not os_name:
        print(f"suture: unsupported platform {system}-{machine}", file=sys.stderr)
        sys.exit(1)
    return f"{os_name}-{arch}"


def find_suture(pkg_dir: str) -> str | None:
    plat = detect_platform()

    # 1. Development mode: local Rust build
    dev_binary = os.path.abspath(os.path.join(pkg_dir, "..", "..", "target", "release", "suture"))
    if os.path.isfile(dev_binary):
        return dev_binary

    # 2. Installed mode: downloaded binary
    installed_binary = os.path.join(pkg_dir, "binaries", plat, "suture")
    if os.path.isfile(installed_binary):
        return installed_binary

    # 3. System PATH
    suture_path = shutil.which("suture")
    if suture_path:
        return suture_path

    return None


def main() -> None:
    pkg_dir = os.path.dirname(os.path.abspath(__file__))
    suture_bin = find_suture(pkg_dir)

    if suture_bin is None:
        print("suture: binary not found.", file=sys.stderr)
        print("", file=sys.stderr)
        print("Install options:", file=sys.stderr)
        print("  1. pip install suture-merge-driver   (downloads prebuilt binary)", file=sys.stderr)
        print("  2. cargo install suture-cli          (build from source)", file=sys.stderr)
        print("  3. Set SUTURE_PATH env var           (use custom binary)", file=sys.stderr)
        sys.exit(1)

    if not sys.argv[1:]:
        print("Usage: suture-merge-driver <base> <ours> <theirs> [path]", file=sys.stderr)
        sys.exit(1)

    result = subprocess.run(
        [suture_bin, "merge-file"] + sys.argv[1:],
    )
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
