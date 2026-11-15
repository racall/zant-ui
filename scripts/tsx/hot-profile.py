#!/usr/bin/env python3
"""Opt-in rustc/linker wall-clock profiling for hot reload (macOS).

RUSTC_WRAPPER=<repo>/scripts/tsx/hot-profile.py ZANT_HOT_PROFILE=<dir> zant run

Only the UI crate uses the linker wrapper. JSON lines include monotonic and
wall timestamps; rustc_ms includes link_ms (subtract when comparing phases).
The normal build does not depend on Python or these wrappers.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def record(phase, start, status):
    directory = Path(os.environ["ZANT_HOT_PROFILE"])
    directory.mkdir(parents=True, exist_ok=True)
    event = dict(phase=phase, ms=(time.perf_counter() - start) * 1000,
                 wall=time.time(), status=status)
    with (directory / "compiler.jsonl").open("a") as stream:
        stream.write(json.dumps(event) + "\n")
    print(f"[zant-hot] {phase}_ms={event['ms']:.3f}", file=sys.stderr)


def main():
    args = sys.argv[1:]
    if Path(sys.argv[0]).name == "hot-linker":
        start = time.perf_counter()
        status = subprocess.call(["cc", *args], close_fds=False)
        record("link", start, status)
        return status
    profile = os.environ.get("ZANT_HOT_PROFILE")
    ui = any(args[i:i + 2] == ["--crate-name", "zant_hot_ui"] for i in range(len(args)))
    if not profile or not ui:
        os.execv(args[0], args)
    directory = Path(profile)
    directory.mkdir(parents=True, exist_ok=True)
    linker = directory / "hot-linker"
    if not linker.exists():
        linker.symlink_to(Path(__file__).resolve())
    start = time.perf_counter()
    status = subprocess.call([*args, "-C", f"linker={linker}"], close_fds=False)
    record("rustc", start, status)
    return status


if __name__ == "__main__":
    sys.exit(main())
