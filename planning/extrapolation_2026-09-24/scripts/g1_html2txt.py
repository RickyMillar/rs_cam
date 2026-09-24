#!/usr/bin/env python3
"""G1 fetch helper: convert a saved HTML page to plain text (one text node per line)."""
import html
import re
import sys

src, dst = sys.argv[1], sys.argv[2]
t = open(src, errors="replace").read()
t = re.sub(r"(?is)<(script|style|noscript).*?</\1>", "", t)
t = re.sub(r"(?i)<br\s*/?>", "\n", t)
t = re.sub(r"(?i)</(td|th)>", "\t", t)
t = re.sub(r"(?i)</(tr|p|div|li|h\d)>", "\n", t)
t = re.sub(r"<[^>]+>", "", t)
t = html.unescape(t)
lines = [re.sub(r"[ \t]+", " ", l).strip() for l in t.splitlines()]
out = "\n".join(l for l in lines if l)
open(dst, "w").write(out + "\n")
