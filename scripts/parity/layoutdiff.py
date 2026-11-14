#!/usr/bin/env python3
"""Compares two ZANT_LAYOUT_FILE dumps for the Zig/Rust parity check.

Layout lines are `key x y width height` separated by tabs (see
`writeLayoutFile` in src/ui_runtime.zig). Lines are aligned by key (not by
position), the key sets must be identical, and every coordinate may differ by
at most --tolerance (default 0.01, plan.md §6).

    python3 scripts/parity/layoutdiff.py a.txt b.txt [--tolerance 0.01]
"""

import argparse
import sys


def load(path):
    boxes = {}
    with open(path, encoding="utf-8") as file:
        for number, line in enumerate(file, 1):
            line = line.rstrip("\n")
            if not line:
                continue
            parts = line.split("\t")
            if len(parts) != 5:
                raise SystemExit(f"{path}:{number}: expected 5 tab-separated fields, got {len(parts)}")
            key = parts[0]
            if key in boxes:
                raise SystemExit(f"{path}:{number}: duplicate key {key!r}")
            boxes[key] = tuple(float(value) for value in parts[1:])
    return boxes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("first")
    parser.add_argument("second")
    parser.add_argument("--tolerance", type=float, default=0.01)
    parser.add_argument("--max-report", type=int, default=20)
    args = parser.parse_args()

    first = load(args.first)
    second = load(args.second)
    problems = []

    missing = sorted(set(first) - set(second))
    extra = sorted(set(second) - set(first))
    for key in missing:
        problems.append(f"missing in {args.second}: {key}")
    for key in extra:
        problems.append(f"missing in {args.first}: {key}")

    for key in sorted(set(first) & set(second)):
        a, b = first[key], second[key]
        line = ", ".join(
            f"{name} {va:.2f} vs {vb:.2f}" for name, va, vb in zip(("x", "y", "w", "h"), a, b) if abs(va - vb) > args.tolerance
        )
        if line:
            problems.append(f"{key}: {line}")

    if problems:
        print(f"MISMATCH {args.first} vs {args.second}: {len(problems)} problem(s)")
        for problem in problems[: args.max_report]:
            print(f"  {problem}")
        if len(problems) > args.max_report:
            print(f"  ... and {len(problems) - args.max_report} more")
        return 1
    print(f"ok {args.first} vs {args.second}: {len(first)} keys identical (±{args.tolerance})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
