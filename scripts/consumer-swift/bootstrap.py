"""Hosted tool installation, all subprocesses admitted and monitored by the same runner."""

import json
import os
import re
import shutil
from pathlib import Path

from receipts import sha


def bootstrap(gate):
    gate.require_head(gate.actions, "fdddc35399a9eb87cb9c35ae87346ef38e7bbc29")
    gate.require_head(gate.poly, "8a85d746736986ce80ae4a2315c15c27e5844d28")
    gate.env["RUSTUP_AUTO_INSTALL"] = "0"
    gate.command(
        "rust-toolchain",
        [
            "rustup",
            "toolchain",
            "install",
            "1.99.1",
            "--profile",
            "minimal",
            "--component",
            "rust-src",
            "--component",
            "rustfmt",
            "--component",
            "clippy",
            "--target",
            "wasm32-unknown-unknown",
            "--no-self-update",
        ],
        gate.alef,
        2 * 1024**3,
        600,
    )
    gate.command("rustup-identity", ["rustup", "--version"], gate.alef, 0, 10)
    sysroot = gate.command(
        "qualified-rust-sysroot", ["rustup", "run", "1.99.1", "rustc", "--print", "sysroot"], gate.alef, 0, 15
    )
    library = Path((sysroot / "stdout.log").read_text().strip()) / "lib"
    if not library.is_dir() or gate.env.get("DYLD_LIBRARY_PATH"):
        raise RuntimeError("matching Rust library missing or overriding loader environment requires review")
    ambient = gate.env.get("DYLD_FALLBACK_LIBRARY_PATH", "")
    gate.env["DYLD_FALLBACK_LIBRARY_PATH"] = str(library) + (os.pathsep + ambient if ambient else "")
    install_task(gate)
    gate.env["POLY_INSTALL_DIR"] = str(gate.bin)
    gate.command(
        "install-poly",
        ["bash", str(gate.poly / "install.sh"), "--no-modify-path", "0.24.0"],
        gate.poly,
        512 * 1024**2,
        600,
    )
    gate.command(
        "install-cargo-sort",
        [gate.real_cargo, "install", "cargo-sort", "--version", "2.0.2", "--locked", "--root", str(gate.tools)],
        gate.alef,
        2 * 1024**3,
        600,
    )
    expected = [
        ("task", "3.54.0"),
        ("poly", "0.24.0"),
        ("cargo-sort", "2.0.2"),
        ("rustc", "1.99.1"),
        ("cargo", "1.99.1"),
    ]
    for program, version in expected:
        output = gate.command("version-" + program, [program, "--version"], gate.alef, 0, 15)
        if version not in (output / "stdout.log").read_text():
            raise RuntimeError("installed tool version differs: " + program)
    cache = gate.command("actual-homebrew-cache", ["brew", "--cache"], gate.xberg, 0, 15)
    actual_cache = Path((cache / "stdout.log").read_text().strip())
    actual_cache.mkdir(parents=True, exist_ok=True)
    actual_cache = actual_cache.resolve(strict=True)
    gate.paths.append(actual_cache)
    gate.publish_paths()
    gate.command(
        "native-dependencies",
        ["bash", str(gate.xberg / "scripts/ci/install-system-deps/install-macos.sh")],
        gate.xberg,
        4 * 1024**3,
        1200,
    )
    install_clang_format(gate)
    for tool in ["cmake", "pkg-config", "swift", "clang", "clang-format"]:
        if not gate.which(tool):
            raise RuntimeError("strict generation/native tool unavailable: " + tool)


def install_task(gate):
    source = gate.env.get("SWIFT_GATE_BOOTSTRAP_TASK")
    receipt_path = gate.env.get("SWIFT_GATE_BOOTSTRAP_RECEIPT")
    if source and receipt_path:
        expected = Path(gate.env["RUNNER_TEMP"]).resolve(strict=True) / "consumer-task-bin/task"
        source = Path(source)
        receipt = json.loads(Path(receipt_path).read_text())
        if source.is_symlink() or source.resolve(strict=True) != expected or receipt.get("version") != "3.54.0":
            raise RuntimeError("unexpected bootstrap Task source/version")
        if sha(source) != receipt["executable_sha256"]:
            raise RuntimeError("bootstrap Task bytes changed after official asset verification")
        shutil.copy2(source, gate.bin / "task")
        (gate.evidence / "reused-task-asset.json").write_text(json.dumps(receipt, indent=2) + "\n")
    elif source or receipt_path:
        raise RuntimeError("incomplete bootstrap Task receipt")
    else:
        gate.command(
            "install-task",
            [gate.python, str(gate.source / "tool_assets.py"), str(gate.bin / "task")],
            gate.actions,
            256 * 1024**2,
            120,
        )


def install_clang_format(gate):
    formatter = gate.tools / "clang-format"
    gate.command("clang-format-venv", [gate.python, "-m", "venv", str(formatter)], gate.alef, 256 * 1024**2, 120)
    binary = formatter / "bin/clang-format"
    report = gate.evidence / "clang-format-install.json"
    gate.command(
        "install-clang-format",
        [
            str(formatter / "bin/python"),
            "-m",
            "pip",
            "--isolated",
            "install",
            "--only-binary=:all:",
            "--index-url",
            "https://pypi.org/simple",
            "--disable-pip-version-check",
            "--no-cache-dir",
            "--report",
            str(report),
            "clang-format==22.1.8",
        ],
        gate.alef,
        512 * 1024**2,
        600,
    )
    gate.env["PATH"] = str(binary.parent) + os.pathsep + gate.env["PATH"]
    selected = gate.which("clang-format")
    if not binary.is_file() or Path(selected or "").resolve() != binary.resolve():
        raise RuntimeError("pinned clang-format executable is not selected on the generation PATH")
    version = gate.command("version-clang-format", [str(binary), "--version"], gate.alef, 0, 15)
    if (version / "stdout.log").read_text().strip() != "clang-format version 22.1.8":
        raise RuntimeError("installed clang-format version differs")
    installed = json.loads(report.read_text())["install"]
    if (
        len(installed) != 1
        or installed[0]["metadata"]["name"] != "clang-format"
        or installed[0]["metadata"]["version"] != "22.1.8"
    ):
        raise RuntimeError("unexpected clang-format package installation report")
    download = installed[0]["download_info"]
    digest = download["archive_info"]["hashes"].get("sha256", "")
    if (
        not download["url"].startswith("https://files.pythonhosted.org/")
        or not download["url"].endswith(".whl")
        or re.fullmatch(r"[0-9a-f]{64}", digest) is None
    ):
        raise RuntimeError("official clang-format wheel provenance missing")
    (gate.evidence / "clang-format-tool.json").write_text(
        json.dumps(
            {
                "selected": str(binary.resolve()),
                "executable_sha256": sha(binary),
                "version": "22.1.8",
                "wheel": download,
            },
            indent=2,
        )
        + "\n"
    )
