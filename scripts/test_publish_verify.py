"""Canonical mise task invocation and archive guards using explicitly synthetic Cargo output."""

import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TASK = Path("mise-tasks/publish/verify.sh")


def require_single_job(argv, target):
    expected = ["package", "--workspace", "--locked", "--target-dir", str(target), "-j1"]
    assert argv == expected, "expected canonical single-job package arguments, observed " + repr(argv)


def invoke_package_fixture(repo, mode="three", wrong_jobs=False):
    with tempfile.TemporaryDirectory(prefix="alef-package-controls-") as directory:
        root = Path(directory).resolve()
        binary = root / "bin"
        binary.mkdir()
        temporary = root / "temporary"
        temporary.mkdir()
        captured = root / "cargo-argv.json"
        shim = binary / "cargo"
        shim.write_text(
            "#!" + sys.executable + "\n"
            "import json,os,sys\nfrom pathlib import Path\n"
            "args=sys.argv[1:]\n"
            "assert args[0]=='package'\n"
            "Path(os.environ['PACKAGE_CONTROL_ARGV']).write_text(json.dumps(args))\n"
            "target=Path(args[args.index('--target-dir')+1]);package=target/'package';package.mkdir()\n"
            "names=['alef-component-abi','alef-component-runtime','alef']\n"
            "mode=os.environ['PACKAGE_CONTROL_MODE']\n"
            "if mode=='two':names=names[:2]\n"
            "for name in names:\n"
            " path=package/(name+'-'+os.environ['PACKAGE_CONTROL_VERSION']+'.crate')\n"
            " path.write_bytes(b'' if mode=='empty' and name=='alef' else b'SYNTHETIC-NOT-A-CRATE')\n"
        )
        shim.chmod(0o755)
        version = tomllib.loads((repo / "Cargo.toml").read_text())["package"]["version"]
        environment = {
            **os.environ,
            "PATH": str(binary) + os.pathsep + os.environ["PATH"],
            "TMPDIR": str(temporary),
            "PACKAGE_CONTROL_ARGV": str(captured),
            "PACKAGE_CONTROL_MODE": mode,
            "PACKAGE_CONTROL_VERSION": version,
            "CARGO_NET_OFFLINE": "true",
        }
        command = ["mise", "run", "publish:verify", version]
        cwd = repo
        if wrong_jobs:
            task = (repo / TASK).read_text()
            assert len(re.findall(r" -j[12]\n", task)) == 1
            task = re.sub(r" -j[12]\n", " -j3\n", task)
            header = "#MISE description="
            task = task.replace(header, "#MISE dir=" + json.dumps(str(repo)) + "\n" + header, 1)
            cwd = root / "shadow"
            (cwd / TASK).parent.mkdir(parents=True)
            (cwd / TASK).write_text(task)
            (cwd / TASK).chmod(0o755)
            (cwd / "mise.toml").write_text("")
            environment["MISE_TRUSTED_CONFIG_PATHS"] = str(cwd)
            environment["MISE_CEILING_PATHS"] = str(root)
        result = subprocess.run(
            command, cwd=cwd, env=environment, capture_output=True, text=True, timeout=15, check=False
        )
        assert captured.is_file(), "canonical task did not invoke the controlled Cargo boundary"
        argv = json.loads(captured.read_text())
        target = Path(argv[argv.index("--target-dir") + 1])
        assert target.is_absolute()
        assert target.is_relative_to(temporary)
        archives = {path.name: path.stat().st_size for path in (target / "package").glob("*.crate")}
        return {
            "exit": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "argv": argv,
            "target": target,
            "archives": archives,
            "version": version,
        }


class PublishVerify(unittest.TestCase):
    def test_canonical_task_uses_one_job_and_three_nonempty_archives(self):
        row = invoke_package_fixture(REPO)
        assert row["exit"] == 0, row
        expected = {
            name + "-" + row["version"] + ".crate" for name in ["alef-component-abi", "alef-component-runtime", "alef"]
        }
        assert set(row["archives"]) == expected
        assert all(size > 0 for size in row["archives"].values())
        assert "Verified 3 nonempty release crate archives" in row["stdout"]
        require_single_job(row["argv"], row["target"])

    def test_wrong_jobs_task_mutant_is_detected_by_the_argument_contract(self):
        row = invoke_package_fixture(REPO, wrong_jobs=True)
        assert row["exit"] == 0
        assert row["argv"][-1] == "-j3"
        try:
            require_single_job(row["argv"], row["target"])
        except AssertionError as error:
            message = str(error)
        else:
            raise AssertionError("wrong jobs argument was accepted")
        assert message.startswith("expected canonical single-job package arguments, observed ")

    def test_two_or_empty_synthetic_archives_fail_the_actual_task_guards(self):
        for mode, message in [
            ("two", "Expected 3 crate archives, found 2"),
            ("empty", "Missing or empty archive for alef"),
        ]:
            with self.subTest(mode=mode):
                row = invoke_package_fixture(REPO, mode=mode)
                assert row["exit"] != 0
                assert message in row["stdout"] + row["stderr"]
                assert len(row["archives"]) == (2 if mode == "two" else 3)


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(PublishVerify)
    assert suite.countTestCases() == 3, "expected 3 publish verification controls"
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
