import json
import tempfile
import unittest
from pathlib import Path

import contracts
from assertions import raises


class Contracts(unittest.TestCase):
    def test_immutable_sources_reject_mutable_ref(self):
        assert contracts.immutable_ref("a" * 40) == "a" * 40
        observed = 0
        try:
            with raises(ValueError):
                pass
        except AssertionError:
            observed += 1
        else:
            raise AssertionError("missing exception was accepted")
        try:
            with raises(ValueError):
                raise TypeError("wrong type must propagate")
        except TypeError as error:
            if str(error) != "wrong type must propagate":
                raise AssertionError from error
            observed += 1
        else:
            raise AssertionError("unrelated exception was swallowed")
        try:
            with raises(ValueError, "expected message"):
                raise ValueError("different message")
        except AssertionError:
            observed += 1
        else:
            raise AssertionError("wrong exception message was accepted")
        assert observed == 3
        for bad in ["main", "v0.111.0", "a" * 39, "A" * 40]:
            with raises(ValueError):
                contracts.immutable_ref(bad)

    def test_forwarded_arguments_keep_features_profile_and_single_lock(self):
        original = ["build", "--release", "--features", "bundled", "--manifest-path", "rust/Cargo.toml"]
        actual = contracts.cargo_arguments(original)
        assert actual[:-3] == original
        assert actual.count("--locked") == 1
        assert contracts.cargo_arguments(["metadata", "--format-version=1"]) == ["metadata", "--format-version=1"]
        with raises(ValueError):
            contracts.cargo_arguments(["check", "--locked", "--locked"])
        with raises(ValueError):
            contracts.cargo_arguments(["build", "--message-format=json"])

    def test_receipt_requires_real_unique_outdir_archive_and_success(self):
        with tempfile.TemporaryDirectory() as root:
            out = Path(root) / "out"
            out.mkdir()
            archive = Path(root) / "libxberg_swift.a"
            archive.write_bytes(b"actual fixture archive marker")
            messages = [
                {"reason": "build-script-executed", "package_id": "binding", "out_dir": str(out)},
                {"reason": "compiler-artifact", "package_id": "binding", "filenames": [str(archive)]},
                {"reason": "build-finished", "success": True},
            ]
            assert contracts.cargo_receipt(map(json.dumps, messages), "binding", True)["out_dir"] == str(out)
            for mutant in [
                messages[:-1],
                messages[1:],
                messages[:1] + messages[2:],
                [*messages[:-1], {"reason": "build-finished", "success": False}],
            ]:
                with raises(ValueError):
                    contracts.cargo_receipt(map(json.dumps, mutant), "binding", True)

    def test_fresh_callback_set_and_missing_or_global_allowance_negative(self):
        swift = '@_cdecl("one$callback")\n@_cdecl("two_callback")'
        lines = "\n".join("cargo:rustc-cdylib-link-arg=-Wl,-U,_" + name for name in ["one$callback", "two_callback"])
        assert contracts.callback_policy(swift, lines) == 2
        for bad in ["", lines.splitlines()[0], lines + "\ndynamic_lookup", lines + "\n" + lines.splitlines()[0]]:
            with raises(ValueError):
                contracts.callback_policy(swift, bad)

    def test_exact_two_safe_definitions_reject_zero_or_extra_extraction(self):
        source = "\n".join("func " + name + "() throws {}" for name in contracts.SAFE_TESTS)
        assert len(contracts.safe_test_census(source)) == 2
        for bad in ["", source + "\nfunc testExtractOCR() {}"]:
            with raises(ValueError):
                contracts.safe_test_census(bad)


if __name__ == "__main__":
    unittest.main()
