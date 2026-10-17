"""Source-only proposed controls; fixture archive is parser input, not native qualification."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import receipts
from assertions import raises


class SelectedBindingReceipt(unittest.TestCase):
    def fixture(self, root, selected_release=True, trailing_release=False):
        root = root.resolve(strict=True)
        target = root / "target"
        out = target / "release/build/xberg-swift-fixture/out"
        nested = out / "xberg-swift"
        nested.mkdir(parents=True)
        for path in [out / "SwiftBridgeCore.swift", out / "SwiftBridgeCore.h", nested / "xberg-swift.h"]:
            path.write_text("fixture\n")
        (nested / "xberg-swift.swift").write_text('@_cdecl("callback")\nfunc callback() {}\n')
        (out.parent / "output").write_text("cargo:rustc-cdylib-link-arg=-Wl,-U,_callback\n")
        archive = target / "release/libxberg_swift.a"
        archive.write_bytes(b"!<arch>\n" + b"parser fixture, not a compiled archive\n" * 40)
        manifest = root / "Cargo.toml"
        manifest.write_text('[package]\nname="xberg-swift"\nversion="0.0.0"\n')
        package = "path+file:///fixture#xberg-swift@0.0.0"
        evidence = root / "evidence"
        build, metadata = evidence / "build", evidence / "metadata"
        build.mkdir(parents=True)
        metadata.mkdir()
        build_args = ["build", "--manifest-path", str(manifest)]
        metadata_args = ["metadata", "--manifest-path", str(manifest)]
        if selected_release:
            build_args.append("--release")
        if trailing_release:
            metadata_args.append("--release")
        for directory, args in [(build, build_args), (metadata, metadata_args)]:
            (directory / "receipt.json").write_text(
                json.dumps(
                    {
                        "forwarded_argv": args,
                        "cwd": str(root),
                        "exit_code": 0,
                        "lock_before": "unchanged",
                        "lock_after": "unchanged",
                    }
                )
            )
        messages = [
            {"reason": "build-finished", "success": True},
            {"reason": "build-script-executed", "package_id": package, "out_dir": str(out)},
            {
                "reason": "compiler-artifact",
                "package_id": package,
                "filenames": [str(archive)],
                "target": {"crate_types": ["staticlib"]},
                "profile": {"test": False},
            },
        ]
        (build / "stdout.log").write_text("\n".join(map(json.dumps, messages)))
        (build / "stderr.log").write_text("Running `rustc -C link-arg=-Wl,-fatal_warnings`\n")
        return evidence, manifest, package, target, out, archive, [build, metadata]

    def invoke(self, root, **kwargs):
        evidence, manifest, package, target, out, archive, order = self.fixture(root, **kwargs)
        original = Path.iterdir

        def ordered(path):
            return iter(order) if path == evidence else original(path)

        with patch.object(Path, "iterdir", ordered):
            result = receipts.binding_receipt(evidence, manifest, package, target, True)
        assert result["out_dir"] == str(out)
        assert result["archives"] == [str(archive)]
        assert result["archive_sha256"] == receipts.sha(archive)
        assert result["callback_count"] == 1

    def test_valid_selected_release_survives_later_metadata_receipt(self):
        with tempfile.TemporaryDirectory() as name:
            try:
                self.invoke(Path(name))
            except RuntimeError as error:
                self.fail("valid selected release rejected: " + str(error))

    def test_other_invocation_release_flag_cannot_qualify_selected_build(self):
        with tempfile.TemporaryDirectory() as name, raises(RuntimeError, "not a unique non-test staticlib artifact"):
            self.invoke(Path(name), selected_release=False, trailing_release=True)
