#!/usr/bin/env python3
"""G4: strip an HTML page to its visible text lines (one line per text node)."""
import html
import re
import sys

src = open(sys.argv[1], errors="ignore").read()
src = re.sub(r"(?is)<(script|style|noscript).*?</\1>", "", src)
src = re.sub(r"(?s)<!--.*?-->", "", src)
src = re.sub(r"(?s)<[^>]+>", "\n", src)
src = html.unescape(src)
lines = [" ".join(l.split()) for l in src.splitlines()]
out = "\n".join(l for l in lines if l)
if len(sys.argv) > 2:
    open(sys.argv[2], "w").write(out + "\n")
else:
    print(out)
