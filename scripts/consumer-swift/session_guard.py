"""Scratch extension of the reviewed Ubuntu observer's owned-child guard."""

import argparse
import json
import os
import shutil
import signal
import subprocess
import time
from pathlib import Path


def session_members(session):
    result = subprocess.run(
        ["/bin/ps", "-axo", "pid=,pgid=,stat="],
        capture_output=True,
        text=True,
        timeout=1,
        check=True,
    )
    members = []
    for line in result.stdout.splitlines():
        pid_text, group_text, state = line.split()
        pid, group = int(pid_text), int(group_text)
        try:
            same_session = os.getsid(pid) == session
        except ProcessLookupError:
            continue
        if same_session and not state.startswith("Z"):
            members.append({"pid": pid, "pgid": group, "state": state})
    return members


def stop_session(child, emit):
    # GroupChild changes PGID but retains SID; rescan SID after reparenting. ~keep
    session = child.pid
    before = session_members(session)
    emit({"event": "cleanup_start", "sid": session, "members": before})
    started = time.monotonic()
    while True:
        members = session_members(session)
        if not members:
            break
        elapsed = time.monotonic() - started
        if elapsed > 5:
            raise TimeoutError(f"owned session cleanup incomplete: {members}")
        selected_signal = signal.SIGTERM if elapsed < 0.5 else signal.SIGKILL
        for group in sorted({member["pgid"] for member in members}):
            # A PID/PGID alone is insufficient evidence for an unrelated process. ~keep
            verified = [member for member in session_members(session) if member["pgid"] == group]
            if not verified:
                continue
            try:
                os.killpg(group, selected_signal)
            except ProcessLookupError:
                continue
            emit(
                {
                    "event": "owned_group_signal",
                    "sid": session,
                    "pgid": group,
                    "signal": int(selected_signal),
                    "members": verified,
                }
            )
        time.sleep(0.05)
    child.wait(timeout=2)
    emit({"event": "cleanup_end", "sid": session, "live_members": session_members(session)})


def supervise(command, root, emit, *, floor, wall, sampler=None, interval=0.1, interrupted=None):
    sampler = sampler or (lambda: shutil.disk_usage(root).free)
    interrupted = interrupted or (lambda: False)
    samples = []

    def sample():
        available = sampler()
        if not isinstance(available, int) or available < 0:
            raise ValueError("disk sample must be a nonnegative byte count")
        samples.append(available)
        emit({"event": "disk_sample", "sequence": len(samples), "free_bytes": available})
        return available

    if sample() < floor:
        emit({"event": "summary", "reason": "disk_limit_before_spawn", "spawned": False})
        return 86
    child = subprocess.Popen(command, cwd=root, start_new_session=True)
    emit({"event": "spawn", "pid": child.pid, "sid": child.pid})
    reason = "child_exit"
    started = time.monotonic()
    try:
        while True:
            if sample() < floor:
                reason = "disk_limit"
                break
            if interrupted():
                reason = "interrupted"
                break
            if time.monotonic() - started >= wall:
                reason = "wall_limit"
                break
            if child.poll() is not None:
                break
            time.sleep(interval)
    finally:
        stop_session(child, emit)
    after = sample()
    if after < floor and reason == "child_exit":
        reason = "disk_limit_after_exit"
    emit(
        {
            "event": "summary",
            "reason": reason,
            "spawned": True,
            "child_exit": child.returncode,
            "samples": len(samples),
            "minimum_free_bytes": min(samples),
            "after_free_bytes": after,
        }
    )
    if reason == "interrupted":
        return 130
    if reason.startswith("disk_limit"):
        return 86
    return 87 if reason == "wall_limit" else child.returncode


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--floor", type=int, default=10 * 1024**3)
    parser.add_argument("--wall", type=float, default=1800)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or not args.root.is_dir() or args.floor <= 0 or args.wall <= 0:
        parser.error("valid owned root, positive budgets and nonempty argv required")
    received = {"signal": None}
    signal.signal(signal.SIGTERM, lambda number, _frame: received.update(signal=number))
    signal.signal(signal.SIGINT, lambda number, _frame: received.update(signal=number))
    return supervise(
        command,
        args.root,
        lambda row: print(json.dumps(row), flush=True),
        floor=args.floor,
        wall=args.wall,
        interrupted=lambda: received["signal"] is not None,
    )


if __name__ == "__main__":
    raise SystemExit(main())
