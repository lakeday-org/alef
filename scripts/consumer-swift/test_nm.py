import hashlib
import unittest
from pathlib import Path

from assertions import raises
from negative import undefined_symbols


class DarwinUndefinedSymbols(unittest.TestCase):
    def fixture(self, name, expected_sha):
        content = (Path(__file__).with_name("fixtures") / name).read_bytes()
        assert hashlib.sha256(content).hexdigest() == expected_sha
        return content.decode()

    def test_actual_preserved_nm_u_logs_have_bare_symbols_and_real_callbacks(self):
        control = self.fixture(
            "darwin-nm-u-control.txt", "de209c950740a6a8e08a5c45ffc3bd51d226422b78093c88de6417ab1a40b3cb"
        )
        assert undefined_symbols(control) == {"___stderrp", "_exit", "_fprintf", "_puts"}
        swift = self.fixture(
            "darwin-swift-dylib-nm-u.txt", "d343ead5e6249afaef6476181a928b2a9e425b34d7dca47bb99eacb43e91f258"
        )
        actual = undefined_symbols(swift)
        assert len(actual) > 2
        assert {name for name in actual if "__swift_bridge__$" in name} == {
            "___swift_bridge__$CounterBox$_free",
            "___swift_bridge__$CounterBox$evaluate",
        }

    def test_archive_member_headers_and_duplicate_references_preserve_symbol_set(self):
        text = "\ncallback_native_probe.callback_native_probe.85140ff8c65c031-cgu.0.rcgu.o:\n_callback\n\nlibprobe.a(next.rcgu.o):\n_callback\n_other\n"
        assert undefined_symbols(text) == {"_callback", "_other"}

    def test_empty_or_header_only_output_cannot_pass_as_zero_work(self):
        for text in ["", "\n\n", "member.o:\n\n", "libprobe.a(member.o):\n"]:
            with self.subTest(text=text), raises(ValueError):
                undefined_symbols(text)

    def test_typed_u_defined_symbols_and_malformed_rows_are_rejected(self):
        for text in [
            "U _callback\n",
            "                 U _callback\n",
            "0000000000000000 T _callback\n",
            "_callback\nunrecognized:\n",
            "_callback unexpected\n",
            "_callback\nerror: failed\n",
        ]:
            with self.subTest(text=text), raises(ValueError):
                undefined_symbols(text)


if __name__ == "__main__":
    unittest.main()
