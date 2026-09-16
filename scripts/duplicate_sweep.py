#!/usr/bin/env python3
"""Semantic duplicate sweep for the rs_cam socraticode index.

Pulls every Rust code chunk vector from the local Qdrant store built by
SocratiCode (Qwen3-Embedding-0.6B, 1024-dim), queries the collection with each
vector, and reports cross-file chunk pairs whose cosine similarity is above a
threshold. High-similarity pairs across different files are the semantic
near-duplicate candidates — tech-debt triage input, not proof.

Usage:
    python3 scripts/duplicate_sweep.py [--threshold 0.93] [--min-len 240]
        [--out report.md] [--max-per-anchor 8] [--src-only]

The report groups the strongest overlapping pairs into clusters, so a chunk
copied into N places appears once, not N times.

Test code is separated from product code twice over:

* a chunk inside an inline `#[cfg(test)] mod` of a `src` file is dropped
  before the search, because a shared test fixture is not product debt;
* a surviving pair counts as a test pair when either side lives under
  `tests/`, `benches/` or `src/bin/`. `--src-only` omits those pairs.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
import urllib.request
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

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


MOD_DECL = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\b")
_STRING_LITERAL = re.compile(
    r"""r\#*"(?:[^"\\]|\\.)*"\#*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)'"""
)


def strip_literals(line: str) -> str:
    """Remove string/char literals and the line comment, so a brace inside
    a `format!` template does not count as code structure."""
    out = _STRING_LITERAL.sub("", line)
    cut = out.find("//")
    return out if cut < 0 else out[:cut]


def skip_attributes(lines: list[str], index: int) -> int | None:
    """Return the index of the first line after `index` that is neither
    blank, nor a comment, nor an attribute. `None` at end of file."""
    total = len(lines)
    while index < total:
        text = lines[index].strip()
        if not text or text.startswith("//"):
            index += 1
            continue
        if text.startswith("#["):
            depth = 0
            while index < total:
                stripped = strip_literals(lines[index])
                depth += stripped.count("[") - stripped.count("]")
                index += 1
                if depth <= 0:
                    break
            continue
        return index
    return None


def module_extent(lines: list[str], index: int) -> tuple[int, int] | None:
    """Brace-match the inline module that starts at `index`.

    Returns the 0-based half-open `(first, last)` line index of the module
    body, or `None` when the declaration is file-backed (`mod tests;`).
    An unclosed body runs to the end of the file.
    """
    total = len(lines)
    depth = 0
    opened = False
    cursor = index
    while cursor < total:
        stripped = strip_literals(lines[cursor])
        if not opened:
            brace = stripped.find("{")
            semicolon = stripped.find(";")
            if semicolon >= 0 and (brace < 0 or semicolon < brace):
                return None
            if brace < 0:
                cursor += 1
                continue
            opened = True
        depth += stripped.count("{") - stripped.count("}")
        if opened and depth <= 0:
            return (index, cursor)
        cursor += 1
    return (index, total - 1)


def test_scopes(path: Path) -> list[tuple[int, int]]:
    """1-based inclusive line ranges of the inline `#[cfg(test)]` modules
    in one Rust file. An empty list means the file has none."""
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError as err:
        print(f"  cannot read {path}: {err}", file=sys.stderr)
        return []
    scopes: list[tuple[int, int]] = []
    for index, raw in enumerate(lines):
        if not raw.strip().startswith("#[cfg(test)]"):
            continue
        head = skip_attributes(lines, index + 1)
        if head is None or not MOD_DECL.match(lines[head].strip()):
            continue
        extent = module_extent(lines, head)
        if extent is None:
            continue
        scopes.append((index + 1, extent[1] + 1))
    return scopes


_SCOPE_CACHE: dict[str, list[tuple[int, int]]] = {}


def in_inline_test_module(rel_path: str, start_line: int) -> bool:
    """True when a `src` chunk starts inside an inline `#[cfg(test)]` module."""
    if "/src/" not in rel_path:
        return False
    scopes = _SCOPE_CACHE.get(rel_path)
    if scopes is None:
        scopes = test_scopes(REPO_ROOT / rel_path)
        _SCOPE_CACHE[rel_path] = scopes
    return any(first <= start_line <= last for first, last in scopes)


def is_test_path(rel_path: str) -> bool:
    """True for a file the workspace compiles as test or bench code."""
    parts = rel_path.split("/")
    return "tests" in parts or "benches" in parts or "src/bin/" in rel_path


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
    ap.add_argument(
        "--src-only",
        action="store_true",
        help="omit pairs whose either side lives under tests/, benches/ or src/bin/",
    )
    args = ap.parse_args()

    print("scrolling chunks...", file=sys.stderr)
    chunks = scroll_all(args.min_len)
    print(f"  {len(chunks)} rust code chunks >= {args.min_len} chars", file=sys.stderr)

    kept = [c for c in chunks if not in_inline_test_module(c.path, c.start)]
    print(
        f"  dropped {len(chunks) - len(kept)} chunks inside inline "
        f"#[cfg(test)] modules; {len(kept)} remain",
        file=sys.stderr,
    )
    chunks = kept

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
    src_pairs = [p for p in pairs if not is_test_path(p[1]) and not is_test_path(p[3])]
    test_count = len(pairs) - len(src_pairs)
    print(f"  {len(pairs)} cross-file pairs >= {args.threshold}", file=sys.stderr)
    print(
        f"  src pairs: {len(src_pairs)} · test pairs: {test_count}",
        file=sys.stderr,
    )
    if args.src_only:
        pairs = src_pairs

    clusters = build_clusters(pairs)

    with open(args.out, "w") as f:
        f.write("# Semantic duplicate sweep — rs_cam\n\n")
        f.write(f"Threshold: cosine >= {args.threshold} · chunks: {len(chunks)} "
                f"(rust, >= {args.min_len} chars, inline `#[cfg(test)]` modules "
                f"dropped) · clusters: {len(clusters)}\n\n")
        f.write(f"src pairs: {len(src_pairs)} · test pairs: {test_count}"
                f"{' (omitted, --src-only)' if args.src_only else ''}\n\n")
        f.write("A pair counts as a test pair when either side lives under "
                "`tests/`, `benches/` or `src/bin/`.\n\n")
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
