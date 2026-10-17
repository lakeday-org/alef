"""Actual native build provenance, not source-cache presence or system-link substitution."""

import json
from pathlib import Path

from receipts import sha


def collect(invocations):
    scripts = []
    for directory in invocations.iterdir():
        for line in (directory / "stdout.log").read_text().splitlines():
            message = json.loads(line)
            if message.get("reason") == "build-script-executed":
                scripts.append(message)
    result = []
    for package in ["xberg-tesseract", "ort-sys"]:
        found = [row for row in scripts if "#" + package + "@" in row["package_id"]]
        if len(found) != 1:
            raise RuntimeError("missing unique actual native build-script receipt: " + package)
        row = found[0]
        out = Path(row["out_dir"]).resolve(strict=True)
        archives = list(out.rglob("*.a"))
        if not archives or len(archives) > 300:
            raise RuntimeError("native static archive provenance missing or unexpectedly broad: " + package)
        native = {str(path.resolve()): {"bytes": path.stat().st_size, "sha256": sha(path)} for path in archives}
        result.append(
            {
                "package_id": row["package_id"],
                "out_dir": str(out),
                "archives": native,
                "linked_libs": row.get("linked_libs"),
                "linked_paths": row.get("linked_paths"),
                "cfgs": row.get("cfgs"),
                "env": row.get("env"),
            }
        )
    tesseract_names = {
        Path(path).name for row in result if "#xberg-tesseract@" in row["package_id"] for path in row["archives"]
    }
    if not {"libtesseract.a", "libleptonica.a"}.issubset(tesseract_names):
        raise RuntimeError("both actual bundled OCR archives required")
    ort_names = {Path(path).name for row in result if "#ort-sys@" in row["package_id"] for path in row["archives"]}
    if "libonnxruntime.a" not in ort_names:
        raise RuntimeError("actual bundled static ORT archive required; system/dynamic replacement rejected")
    return result
