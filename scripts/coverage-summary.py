#!/usr/bin/env python3
"""Parse cargo-llvm-cov JSON output and print a coverage summary."""

import json
import sys

def main():
    if len(sys.argv) < 2:
        print("Usage: coverage-summary.py <coverage.json>", file=sys.stderr)
        sys.exit(1)

    with open(sys.argv[1]) as f:
        data = json.load(f)

    items = data.get("data", [])
    for item in reversed(items):
        totals = item.get("totals", {})
        funcs = totals.get("functions", {})
        lines_t = totals.get("lines", {})
        regions = totals.get("regions", {})

        func_count = funcs.get("count", 0)
        if func_count == 0:
            continue

        print(f"Functions: {funcs.get('covered', 0)}/{func_count} ({funcs.get('percent', 0):.1f}%)")
        print(f"Lines:     {lines_t.get('covered', 0)}/{lines_t.get('count', 0)} ({lines_t.get('percent', 0):.1f}%)")
        print(f"Regions:   {regions.get('covered', 0)}/{regions.get('count', 0)} ({regions.get('percent', 0):.1f}%)")
        break

if __name__ == "__main__":
    main()
