#!/usr/bin/env python3
"""G10 (entry parameters): check the fetch statements against the stored texts.

Reads every fetch/G10/parts/*_sources.json and *_statements.json (or the
merged fetch/G10/sources.json and statements.json when --merged is given).
For each statement it asserts:
  - the source_id exists in a sources file;
  - the stored text file exists, and its sha256 matches `text_sha256`;
  - the `verbatim` string occurs in the stored text (whitespace is
    collapsed on both sides before the comparison, nothing else);
  - every entry in `derived` carries a `formula` (a derived value is never
    unmarked).
It prints one line per failure and a count per part. Exit code 1 on any
failure.

Usage: python3 g10_check_statements.py [--merged]
"""
import glob
import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
G10 = os.path.normpath(os.path.join(HERE, "..", "fetch", "G10"))


def norm(s):
    return re.sub(r"\s+", " ", s).strip()


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()


def load_pairs(merged):
    if merged:
        return [("merged", os.path.join(G10, "sources.json"), os.path.join(G10, "statements.json"))]
    pairs = []
    for sp in sorted(glob.glob(os.path.join(G10, "parts", "*_sources.json"))):
        part = os.path.basename(sp)[: -len("_sources.json")]
        pairs.append((part, sp, os.path.join(G10, "parts", part + "_statements.json")))
    return pairs


def main():
    merged = "--merged" in sys.argv
    failures = 0
    sources = {}
    pairs = load_pairs(merged)
    for part, sp, _ in pairs:
        for s in json.load(open(sp)):
            if s["source_id"] in sources:
                print(f"[{part}] duplicate source_id {s['source_id']}")
                failures += 1
            sources[s["source_id"]] = s
    text_cache = {}
    for sid, s in sources.items():
        rel = s.get("stored_text")
        if not rel:
            continue  # a dead end carries no text
        path = os.path.join(G10, rel)
        if not os.path.exists(path):
            print(f"[{sid}] stored text missing: {rel}")
            failures += 1
            continue
        got = sha256(path)
        if s.get("text_sha256") and s["text_sha256"] != got:
            print(f"[{sid}] text_sha256 mismatch: recorded {s['text_sha256'][:16]} actual {got[:16]}")
            failures += 1
        text_cache[sid] = norm(open(path, encoding="utf-8", errors="replace").read())
    for part, _, stp in pairs:
        if not os.path.exists(stp):
            print(f"[{part}] no statements file (sources only)")
            continue
        sts = json.load(open(stp))
        bad = 0
        for st in sts:
            sid = st.get("source_id")
            if sid not in sources:
                print(f"[{part}] {st.get('statement_id')}: unknown source_id {sid}")
                bad += 1
                continue
            if sid not in text_cache:
                print(f"[{part}] {st.get('statement_id')}: source {sid} has no stored text")
                bad += 1
                continue
            v = norm(st.get("verbatim", ""))
            if not v or v not in text_cache[sid]:
                print(f"[{part}] {st.get('statement_id')}: verbatim not in {sid}: {v[:80]!r}")
                bad += 1
            for d in st.get("derived", []) or []:
                if not d.get("formula"):
                    print(f"[{part}] {st.get('statement_id')}: derived value without formula: {d}")
                    bad += 1
        print(f"[{part}] {len(sts)} statements, {bad} failures")
        failures += bad
    print(f"sources: {len(sources)}; failures: {failures}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
