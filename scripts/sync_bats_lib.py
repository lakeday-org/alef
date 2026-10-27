"""Synchronize the pinned shared Bats helper used locally and in CI."""

import argparse
import pathlib
import urllib.request

ACTIONS_REVISION = "4e36aa5b2afa27df9118cf6f6f84b8f56d8d0cd6"


def main() -> None:
    """Download the reviewed helper, or check the committed copy for drift."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    url = f"https://raw.githubusercontent.com/xberg-io/actions/{ACTIONS_REVISION}/bats-lib/xberg-bats/load.bash"
    with urllib.request.urlopen(url, timeout=30) as response:  # noqa: S310  # ~keep Fixed HTTPS origin and immutable revision.
        expected = response.read()
    if b"XBERG_BATS_LIB_VERSION=1" not in expected:
        raise SystemExit("Downloaded Bats helper does not expose the expected API")
    destination = pathlib.Path(__file__).resolve().parent.parent / "tests/lib/xberg-bats/load.bash"
    if arguments.check:
        if not destination.is_file() or destination.read_bytes() != expected:
            raise SystemExit("Shared Bats helper drifted; run task test:lib:sync")
        print("Checked 1 vendored Bats helper against the pinned source")
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(expected)
        print("Synchronized 1 vendored Bats helper")


if __name__ == "__main__":
    main()
