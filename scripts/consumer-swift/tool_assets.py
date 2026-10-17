"""Verify installed Task bytes against the official versioned GitHub release digest."""

import hashlib
import io
import json
import platform
import sys
import tarfile
import urllib.request
from pathlib import Path


def bounded(url, maximum):
    if not url.startswith("https://"):
        raise ValueError("HTTPS-only public asset")
    with urllib.request.urlopen(url, timeout=30) as response:
        data = response.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError("bounded public asset exceeds allowance")
    return data


def task_asset():
    architecture = {"arm64": "arm64", "x86_64": "amd64"}.get(platform.machine())
    if architecture is None:
        raise ValueError("unsupported hosted Task architecture")
    metadata = json.loads(bounded("https://api.github.com/repos/go-task/task/releases/tags/v3.54.0", 2 * 1024**2))
    name = "task_darwin_" + architecture + ".tar.gz"
    matches = [asset for asset in metadata["assets"] if asset["name"] == name]
    if len(matches) != 1 or not matches[0].get("digest", "").startswith("sha256:"):
        raise ValueError("unique official Task archive and GitHub SHA256 digest required")
    asset = matches[0]
    data = bounded(asset["browser_download_url"], 32 * 1024**2)
    actual = hashlib.sha256(data).hexdigest()
    if "sha256:" + actual != asset["digest"] or len(data) != asset["size"]:
        raise ValueError("official Task asset digest or exact length differs")
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        members = archive.getmembers()
        if len(members) > 50 or sum(member.size for member in members) > 64 * 1024**2:
            raise ValueError("Task asset archive bounds exceeded")
        member = archive.getmember("task")
        if not member.isfile():
            raise ValueError("Task executable must be regular archive member")
        executable = archive.extractfile(member).read()
    expected = hashlib.sha256(executable).hexdigest()
    return executable, {
        "asset_id": asset["id"],
        "asset_sha256": actual,
        "executable_sha256": expected,
        "version": "3.54.0",
        "architecture": architecture,
    }


def install_task(installed):
    executable, receipt = task_asset()
    installed = Path(installed)
    with installed.open("xb") as stream:
        stream.write(executable)
    installed.chmod(0o755)
    return receipt


if __name__ == "__main__":
    print(json.dumps(install_task(sys.argv[1])))
