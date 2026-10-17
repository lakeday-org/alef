"""Source-only hosted consumer contracts; no build is launched by this module."""

import json
import re
import shutil
from pathlib import Path

FLOOR = 10 * 1024**3 + 64 * 1024**2
SAFE_TESTS = {
    "testCacheStatsRoundTripsThroughJSON",
    "testLegacyConfigInitializersRemainSourceCompatible",
}


def immutable_ref(value):
    if re.fullmatch(r"[0-9a-f]{40}", value) is None:
        raise ValueError("expected immutable lowercase forty-character commit")
    return value


def cargo_arguments(arguments):
    result = list(arguments)
    if not result or result[0] not in {"build", "check"}:
        return result
    if result.count("--locked") > 1:
        raise ValueError("duplicate lock policy")
    if "--locked" not in result:
        result.append("--locked")
    if any(arg.startswith("--message-format") for arg in result):
        raise ValueError("unexpected caller message format; review rather than overwrite")
    result.append("--message-format=json-render-diagnostics")
    if "-vv" not in result and "--verbose" not in result and "-v" not in result:
        result.append("-vv")
    return result


def cargo_receipt(lines, package_id, release):
    messages = [json.loads(line) for line in lines if line.strip()]
    builds = [m for m in messages if m.get("reason") == "build-finished"]
    if len(builds) != 1 or builds[0].get("success") is not True:
        raise ValueError("missing unique successful Cargo build completion")
    outputs = {
        m["out_dir"]
        for m in messages
        if m.get("reason") == "build-script-executed" and m.get("package_id") == package_id
    }
    if len(outputs) != 1:
        raise ValueError("missing unique actual binding build-script OUT_DIR")
    out_dir = Path(outputs.pop()).resolve(strict=True)
    archives = {
        Path(name).resolve(strict=True)
        for m in messages
        if m.get("reason") == "compiler-artifact" and m.get("package_id") == package_id
        for name in m.get("filenames", [])
        if name.endswith(".a")
    }
    if release and len(archives) != 1:
        raise ValueError("missing unique actual binding static archive")
    return {"out_dir": str(out_dir), "archives": sorted(map(str, archives))}


def callback_exports(swift):
    names = re.findall(r'@_cdecl\("([^"\n]+)"\)', swift)
    if not names or len(names) != len(set(names)):
        raise ValueError("empty or duplicate Swift callback set")
    if any(re.fullmatch(r"[A-Za-z_$][A-Za-z0-9_$]*", name) is None for name in names):
        raise ValueError("unsafe callback symbol")
    return set(names)


def callback_policy(swift, directives):
    expected = {"cargo:rustc-cdylib-link-arg=-Wl,-U,_" + name for name in callback_exports(swift)}
    actual = [line for line in directives.splitlines() if "rustc-cdylib-link-arg" in line]
    if len(actual) != len(expected) or set(actual) != expected:
        raise ValueError("callback allowance set differs from real Swift declarations")
    if "dynamic_lookup" in directives:
        raise ValueError("global undefined-symbol allowance")
    return len(expected)


def safe_test_census(swift_source):
    actual = re.findall(r"func\s+(test\w+)\s*\(", swift_source)
    if len(actual) != 2 or set(actual) != SAFE_TESTS:
        raise ValueError("safe package test definitions changed; review before execution")
    return actual


def live_space(paths, growth_cap):
    if growth_cap <= 0:
        raise ValueError("growth cap must be positive")
    free = {str(Path(path).resolve(strict=True)): shutil.disk_usage(path).free for path in paths}
    if not free or min(free.values()) < FLOOR + growth_cap:
        raise ValueError("insufficient actual free space across execution/cache volumes")
    return free
