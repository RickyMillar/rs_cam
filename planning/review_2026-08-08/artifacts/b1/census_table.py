#!/usr/bin/env python3
"""Turn a B-1 `calls.jsonl` into the read-size census table (markdown)."""

import json
import sys


def main(path):
    rows = []
    events = []
    for line in open(path):
        d = json.loads(line)
        if "call" in d:
            rows.append(d)
        else:
            events.append(d)

    print("| # | call | params | response bytes (tool text) | JSON-RPC line bytes | wall s |")
    print("|---|---|---|---|---|---|")
    for i, r in enumerate(rows, 1):
        args = json.dumps(r["args"], separators=(",", ":")) if r["args"] else "(none)"
        cb = r.get("content_bytes")
        lb = r.get("line_bytes")
        cb = f"{cb:,}" if isinstance(cb, int) else f"— ({r.get('transport')})"
        lb = f"{lb:,}" if isinstance(lb, int) else "—"
        print(f"| {i} | `{r['call']}` | `{args}` | {cb} | {lb} | {r['wall_s']} |")

    print()
    print("Biggest responses:")
    sized = [r for r in rows if isinstance(r.get("content_bytes"), int)]
    sized.sort(key=lambda r: -r["content_bytes"])
    for r in sized[:12]:
        print(f"  {r['content_bytes']:>12,}  {r['wall_s']:>8}s  {r['call']} "
              f"{json.dumps(r['args'], separators=(',', ':'))}")
    print()
    print("Events:")
    for e in events:
        e2 = dict(e)
        e2.pop("raw", None)
        print("  " + json.dumps(e2)[:400])


if __name__ == "__main__":
    main(sys.argv[1])
