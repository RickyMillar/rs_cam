#!/usr/bin/env python3
"""G5 fetch helper: convert a stored HTML page to plain text.

Usage: g5_html_to_text.py <in.html> <out.txt> <source_url>
The script removes script, style and nav elements and keeps the visible
text, one block per line. It writes a header with the URL and the sha256
of the raw HTML. The sha256 of the text file is computed separately.
"""
import hashlib
import re
import sys

from bs4 import BeautifulSoup

src, dst, url = sys.argv[1], sys.argv[2], sys.argv[3]
raw = open(src, "rb").read()
soup = BeautifulSoup(raw, "html.parser")
for tag in soup(["script", "style", "noscript", "svg", "head"]):
    tag.decompose()
for br in soup.find_all("br"):
    br.replace_with("\n")
for cell in soup.find_all(["td", "th"]):
    cell.append(" | ")
for row in soup.find_all(["tr", "p", "li", "div", "h1", "h2", "h3", "h4", "h5", "table"]):
    row.append("\n")
text = soup.get_text()
lines = [re.sub(r"[ \t\xa0]+", " ", ln).strip() for ln in text.splitlines()]
out, prev_blank = [], False
for ln in lines:
    if not ln:
        if not prev_blank:
            out.append("")
        prev_blank = True
        continue
    out.append(ln)
    prev_blank = False
with open(dst, "w") as fh:
    fh.write(f"# source_url: {url}\n# accessed_on: 2026-09-24\n")
    fh.write(f"# raw_html_sha256: {hashlib.sha256(raw).hexdigest()}\n\n")
    fh.write("\n".join(out).strip() + "\n")
