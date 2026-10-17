"""Pinned formatter setup through actual bootstrap at its owned command boundary."""

import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import bootstrap


class FormatterGate:
    def __init__(self, root, wrong_version=False, wrong_path=False, invalid_report=None):
        self.alef = root / "alef"
        self.xberg = root / "xberg"
        self.actions = root / "actions"
        self.poly = root / "poly"
        self.evidence = root / "evidence"
        self.tools = self.evidence / "tools"
        self.bin = self.tools / "bin"
        self.env = {"PATH": "/owned/ambient"}
        self.real_cargo = "/owned/cargo"
        self.python = "/owned/python"
        self.paths = []
        self.calls = []
        self.wrong_version = wrong_version
        self.wrong_path = wrong_path
        self.invalid_report = invalid_report
        for path in [self.alef, self.xberg, self.evidence, self.bin, root / "sysroot/lib", root / "brew-cache"]:
            path.mkdir(parents=True, exist_ok=True)

    def require_head(self, _root, _sha):
        pass

    def publish_paths(self):
        pass

    def which(self, tool):
        if tool != "clang-format":
            return "/owned/" + tool
        binary = self.tools / "clang-format/bin/clang-format"
        if self.wrong_path:
            return "/foreign/clang-format"
        if binary.is_file() and self.env["PATH"].split(os.pathsep)[0] == str(binary.parent):
            return str(binary)
        return None

    def command(self, name, argv, cwd, budget, timeout):
        self.calls.append({"name": name, "argv": argv, "cwd": cwd, "budget": budget, "timeout": timeout})
        result = self.evidence / name
        result.mkdir()
        output = ""
        if name == "qualified-rust-sysroot":
            output = str(self.alef.parent / "sysroot")
        elif name == "actual-homebrew-cache":
            output = str(self.alef.parent / "brew-cache")
        elif name.startswith("version-"):
            output = {
                "task": "3.54.0",
                "poly": "0.24.0",
                "cargo-sort": "2.0.2",
                "rustc": "1.99.1",
                "cargo": "1.99.1",
                "clang-format": "clang-format version " + ("18.1.8" if self.wrong_version else "22.1.8"),
            }[name.removeprefix("version-")]
        elif name == "install-clang-format":
            binary = self.tools / "clang-format/bin/clang-format"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"owned boundary fixture, not a native executable")
            report = self.evidence / "clang-format-install.json"
            download = {
                "url": "https://files.pythonhosted.org/fixture.whl",
                "archive_info": {"hashes": {"sha256": "a" * 64}},
            }
            if self.invalid_report == "digest":
                download["archive_info"]["hashes"]["sha256"] = "not-a-sha256"
            elif self.invalid_report == "host":
                download["url"] = "https://files.pythonhosted.org.attacker.invalid/fixture.whl"
            elif self.invalid_report == "suffix":
                download["url"] = "https://files.pythonhosted.org/fixture.tar.gz"
            report.write_text(
                json.dumps(
                    {
                        "install": [
                            {"metadata": {"name": "clang-format", "version": "22.1.8"}, "download_info": download}
                        ]
                    }
                )
            )
        (result / "stdout.log").write_text(output + "\n")
        return result


class FormatterBootstrap(unittest.TestCase):
    def test_missing_ambient_formatter_is_installed_pinned_and_selected_before_strict_check(self):
        with tempfile.TemporaryDirectory() as directory:
            gate = FormatterGate(Path(directory))
            with patch.object(bootstrap, "install_task", lambda _gate: None):
                try:
                    bootstrap.bootstrap(gate)
                except RuntimeError as error:
                    self.fail("pinned formatter setup did not complete: " + str(error))
            calls = {row["name"]: row for row in gate.calls}
            install = calls["install-clang-format"]
            assert "clang-format==22.1.8" in install["argv"]
            assert "--only-binary=:all:" in install["argv"]
            assert install["argv"][install["argv"].index("--index-url") + 1] == "https://pypi.org/simple"
            assert (install["budget"], install["timeout"]) == (512 * 1024**2, 600)
            tool = json.loads((gate.evidence / "clang-format-tool.json").read_text())
            assert tool["selected"] == gate.which("clang-format")
            assert tool["version"] == "22.1.8"

    def test_wrong_formatter_version_is_rejected_after_actual_version_command(self):
        with tempfile.TemporaryDirectory() as directory:
            gate = FormatterGate(Path(directory), wrong_version=True)
            with patch.object(bootstrap, "install_task", lambda _gate: None):
                try:
                    bootstrap.bootstrap(gate)
                except RuntimeError as error:
                    message = str(error)
                else:
                    raise AssertionError("wrong formatter version was accepted")
            assert message == "installed clang-format version differs", message
            assert gate.calls[-1]["name"] == "version-clang-format"

    def test_foreign_path_cannot_satisfy_the_pinned_formatter_setup(self):
        for scenario in ["path", "digest", "host", "suffix"]:
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as directory:
                gate = FormatterGate(Path(directory), wrong_path=scenario == "path", invalid_report=scenario)
                with patch.object(bootstrap, "install_task", lambda _gate: None):
                    try:
                        bootstrap.bootstrap(gate)
                    except RuntimeError as error:
                        message = str(error)
                    else:
                        raise AssertionError("foreign formatter executable was accepted")
                expected = (
                    "pinned clang-format executable is not selected on the generation PATH"
                    if scenario == "path"
                    else "official clang-format wheel provenance missing"
                )
                assert message == expected, message
                expected_stage = "install-clang-format" if scenario == "path" else "version-clang-format"
                assert gate.calls[-1]["name"] == expected_stage
