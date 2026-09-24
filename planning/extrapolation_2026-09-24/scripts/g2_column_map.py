#!/usr/bin/env python3
"""G2 helper: map each value on a pdftotext -layout chart row to the header
column whose centre is nearest (by character position).
Usage: g2_column_map.py text_file header_line_no row_line_no [row_line_no ...]
Line numbers are 1-based. The output is a reading aid; check it against a
rendered image of the page before a value is transcribed."""
import re, sys
lines = open(sys.argv[1], encoding="utf-8", errors="replace").read().split("\n")
hdr = lines[int(sys.argv[2]) - 1].expandtabs(8)
cols = []
for m in re.finditer(r"(\d+(?:[ -]\d+/\d+|/\d+)?)", hdr):
    if m.start() < 15:
        continue
    cols.append((m.group(1), (m.start() + m.end()) / 2))
for ln in sys.argv[3:]:
    row = lines[int(ln) - 1].expandtabs(8)
    out = []
    for m in re.finditer(r"\.\d{3} ?-\s?\.\d{3}\*{0,2}", row):
        c = (m.start() + m.end()) / 2
        name, pos = min(cols, key=lambda k: abs(k[1] - c))
        out.append(f"{name}:{m.group(0)} (off {c - pos:+.0f})")
    print(ln, row[:14].strip(), " | ".join(out))
