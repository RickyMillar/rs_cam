#!/usr/bin/env python3
"""Semantic duplicate sweep for the rs_cam socraticode index.

Pulls every Rust code chunk vector from the local Qdrant store built by
SocratiCode (Qwen3-Embedding-0.6B, 1024-dim), queries the collection with each
vector, and reports cross-file chunk pairs whose cosine similarity is above a
threshold. High-similarity pairs across different files are the semantic
near-duplicate candidates — tech-debt triage input, not proof.

Usage:
    python3 scripts/duplicate_sweep.py [--threshold 0.93] [--min-len 240]
        [--out report.md] [--max-per-anchor 8]

The report groups the strongest overlapping pairs into clusters, so a chunk
copied into N places appears once, not N times.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.request
from collections import defaultdict
from dataclasses import dataclass

QDRANT = "http://localhost:16333"
COLLECTION = "codebase_a3510da6358c"
ANCHOR_FILTER = {"must": [
    {"key": "language", "match": {"value": "rust"}},
    {"key": "type", "match": {"value": "code"}},
]}


def post(path: str, body: dict) -> dict:
    req = urllib.request.Request(
        QDRANT + path,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=300) as r:
        return json.loads(r.read())


@dataclass
class Chunk:
    pid: str
    path: str
    start: int
    end: int
    content: str
    vec: list[float]


def scroll_all(min_len: int) -> list[Chunk]:
    chunks: list[Chunk] = []
    offset = None
    while True:
        body: dict = {
            "limit": 256,
            "with_payload": True,
            "with_vector": ["dense"],
            "filter": ANCHOR_FILTER,
        }
        if offset is not None:
            body["offset"] = offset
        res = post(f"/collections/{COLLECTION}/points/scroll", body)["result"]
        for p in res["points"]:
            pl = p["payload"]
            if len(pl.get("content", "")) < min_len:
                continue
            vec = p["vector"]
            if isinstance(vec, dict):
                vec = vec["dense"]
            chunks.append(Chunk(p["id"], pl["relativePath"], pl["startLine"],
                                pl["endLine"], pl["content"], vec))
        offset = res.get("next_page_offset")
        if offset is None:
            break
    return chunks


def batch_search(anchors: list[Chunk], limit_per: int) -> dict[str, list[tuple[str, str, float]]]:
    """Return anchor pid -> [(match pid, match path, score)] for cross-file hits."""
    hits: dict[str, list[tuple[str, str, float]]] = defaultdict(list)
    path_by_id = {c.pid: c.path for c in anchors}
    B = 32
    total = len(anchors)
    t0 = time.time()
    for i in range(0, total, B):
        batch = anchors[i:i + B]
        res = post(f"/collections/{COLLECTION}/points/query/batch", {
            "searches": [
                {
                    "query": c.vec,
                    "limit": limit_per,
                    "using": "dense",
                    "filter": ANCHOR_FILTER,
                }
                for c in batch
            ],
        })
        for anchor, r in zip(batch, res["result"]):
            for m in r["points"]:
                mid = m["id"]
                if mid == anchor.pid:
                    continue
                mp = path_by_id.get(mid)
                if mp is None or mp == anchor.path:
                    continue  # skip unindexed / same-file neighbours
                hits[anchor.pid].append((mid, mp, m["score"]))
        done = min(i + B, total)
        if done % 2560 == 0 or done == total:
            rate = done / max(time.time() - t0, 1e-9)
            print(f"  {done}/{total} anchors ({rate:.0f}/s)", file=sys.stderr)
    return hits


def build_clusters(pairs: list[tuple[float, str, str, str, str]]) -> list[list]:
    """Union-find over file paths; return clusters of pairs sorted by score."""
    parent: dict[str, str] = {}

    def find(x: str) -> str:
        parent.setdefault(x, x)
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    def union(a: str, b: str) -> None:
        ra, rb = find(a), find(b)
        if ra != rb:
            parent[ra] = rb

    for _, pa, _, pb, _ in pairs:
        union(pa, pb)

    clusters: dict[str, list] = defaultdict(list)
    for p in pairs:
        clusters[find(p[1])].append(p)
    return sorted(clusters.values(), key=lambda c: -max(x[0] for x in c))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--threshold", type=float, default=0.93)
    ap.add_argument("--min-len", type=int, default=240)
    ap.add_argument("--limit-per-anchor", type=int, default=8)
    ap.add_argument("--out", default="duplicate_sweep_report.md")
    args = ap.parse_args()

    print("scrolling chunks...", file=sys.stderr)
    chunks = scroll_all(args.min_len)
    print(f"  {len(chunks)} rust code chunks >= {args.min_len} chars", file=sys.stderr)

    print("querying...", file=sys.stderr)
    hits = batch_search(chunks, args.limit_per_anchor)

    # Build pair records above threshold.
    pair_set: set[frozenset] = set()
    pairs = []
    for aid, matches in hits.items():
        a = next(c for c in chunks if c.pid == aid)
        for mid, mpath, score in matches:
            if score < args.threshold:
                continue
            key = frozenset((aid, mid))
            if key in pair_set:
                continue
            pair_set.add(key)
            m = next(c for c in chunks if c.pid == mid)
            pairs.append((score, a.path, f"L{a.start}-{a.end}",
                         m.path, f"L{m.start}-{m.end}"))
    pairs.sort(key=lambda p: -p[0])
    print(f"  {len(pairs)} cross-file pairs >= {args.threshold}", file=sys.stderr)

    clusters = build_clusters(pairs)

    with open(args.out, "w") as f:
        f.write("# Semantic duplicate sweep — rs_cam\n\n")
        f.write(f"Threshold: cosine >= {args.threshold} · chunks: {len(chunks)} "
                f"(rust, >= {args.min_len} chars) · "
                f"clusters: {len(clusters)}\n\n")
        f.write("Near-duplicate candidates across different files. Semantic "
                "similarity, not proof: shared boilerplate, trait impls with "
                "the same shape, and intentional parallels all appear here.\n\n---\n\n")
        for i, cl in enumerate(clusters, 1):
            top = cl[0]
            files = sorted({p[1] for p in cl} | {p[3] for p in cl})
            f.write(f"## Cluster {i} — top score {top[0]:.4f} ({len(files)} files, "
                    f"{len(cl)} pairs)\n\n")
            f.write("Files:\n")
            for fp in files:
                f.write(f"- `{fp}`\n")
            f.write("\nStrongest pairs:\n\n")
            for score, pa, la, pb, lb in cl[:5]:
                f.write(f"- **{score:.4f}** `{pa}` {la} ↔ `{pb}` {lb}\n")
            if len(cl) > 5:
                f.write(f"- … {len(cl) - 5} more pairs\n")
            f.write("\n")
    print(f"wrote {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
