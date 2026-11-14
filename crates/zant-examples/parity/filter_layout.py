#!/usr/bin/env python3
"""Normalizes a ZANT_LAYOUT_FILE dump for the zant-components parity check.

The Zig `ui.pdfViewer` writes its outer key twice (`pdf-2002`), which
`scripts/parity/layoutdiff.py` rejects as a duplicate; and the PDF viewer is
not ported yet (WP-3.9), so its inner keys exist only on the Zig side.

    filter_layout.py <in.txt> <out.txt> [--drop-prefix pdf-]

- keeps the first line of a duplicated key
- drops the keys starting with --drop-prefix (default: none)

Used by `crates/zant-examples/parity/components_parity.sh`.
"""

import argparse
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source")
    parser.add_argument("target")
    parser.add_argument("--drop-prefix", default="")
    args = parser.parse_args()

    seen = set()
    lines = []
    with open(args.source, encoding="utf-8") as source:
        for number, line in enumerate(source, 1):
            line = line.rstrip("\n")
            if not line:
                continue
            key = line.split("\t")[0]
            if key in seen:
                continue
            seen.add(key)
            if args.drop_prefix and key.startswith(args.drop_prefix):
                continue
            lines.append(line + "\n")
    with open(args.target, "w", encoding="utf-8") as target:
        target.writelines(lines)
    print(f"kept {len(lines)} unique keys from {args.source}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
