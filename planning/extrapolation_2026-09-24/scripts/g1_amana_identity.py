#!/usr/bin/env python3
"""G1 fetch helper: record the geometry of each Amana tool number that the
ZrN 2D/3D carving chart lists.

The chart prints a chipload per column ("1mm", "1/32 - 1mm", ...) and lists
the tool numbers in each section. The chart does not say which tool numbers
are tapered. This script fetches one distributor product page per number and
copies the spec block (Diameter, Radius, Angle, Flutes, ...) verbatim.

Output: <out_dir>/amana_46xxx_identity.txt (the text that the candidate rows
cite) and the raw pages under <raw_dir>/ with their sha256 in the text.
"""
import hashlib
import html
import re
import subprocess
import sys
import urllib.parse

out_dir, raw_dir = sys.argv[1], sys.argv[2]
UA = "Mozilla/5.0 (X11; Linux x86_64) Chrome/120"

# Every tool number that either chart version lists, per chart section.
SECTIONS = {
    "2 Flute Ball Nose": ["46252", "46256", "46471", "46283", "46285", "46289",
                          "46294", "46479"],
    "3 Flute Ball Nose": ["46280", "46281", "46284", "46286", "46287", "46288",
                          "46291", "46295", "46298", "46470", "46471", "46473",
                          "46474", "46494", "46495", "46580"],
    "3 Flute Extra Long Ball Nose & Flat Bottom": ["46490", "46491", "46493",
                                                  "46496", "46590", "46591",
                                                  "46593", "46596"],
    "4 Flute Ball Nose & Flat Bottom": ["46282", "46292", "46293", "46472",
                                        "46572", "46582", "46583", "46584"],
}
# Product pages found by web search where the distributor search is empty.
FALLBACK = {
    "46298": "https://toolstoday.com/v-12357-46298.html",
    "46494": "https://www.toolstoday.com/v-12287-46494.html",
    "46495": "https://toolstoday.com/v-12083-46495.html",
    "46493": "https://toolstoday.com/v-11871-46493.html",
    "46580": "https://www.arrowtooling.com/index.php?l=product_detail&p=32359",
}


def get(url, path):
    subprocess.run(["curl", "-sL", "-A", UA, "-o", path, url], check=False)
    try:
        return open(path, "rb").read()
    except OSError:
        return b""


def text_of(raw):
    t = raw.decode("utf-8", errors="replace")
    t = re.sub(r"(?is)<(script|style|noscript).*?</\1>", "", t)
    t = re.sub(r"(?i)<br\s*/?>", "\n", t)
    t = re.sub(r"(?i)</(td|th)>", "\t", t)
    t = re.sub(r"(?i)</(tr|p|div|li|h\d|dt|dd)>", "\n", t)
    t = re.sub(r"<[^>]+>", "", t)
    t = html.unescape(t)
    lines = [re.sub(r"[ \t]+", " ", l).strip() for l in t.splitlines()]
    return [l for l in lines if l]


SPEC_KEYS = ("Diameter (D)", "Radius (R)", "Angle (a", "Flutes ",
             "Cutting Height", "Shank (d)", "Overall Length")

out = ["# Amana ZrN 2D/3D carving chart: tool identity per listed tool number",
       "# Accessed 2026-09-24. Each block: the page URL, the sha256 of the raw",
       "# page, the product title and the spec lines, copied verbatim.",
       ""]
seen = set()
for section, nums in SECTIONS.items():
    out.append(f"## chart section: {section}")
    for n in nums:
        if (section, n) in seen:
            continue
        seen.add((section, n))
        url = None
        s_raw = get("https://fastoolnow.com/search.php?search_query=" + n,
                    f"{raw_dir}/fsearch_{n}.html").decode("utf-8", "replace")
        m = re.search(r'https://fastoolnow\.com/amana-' + n + r'-[^"?]*', s_raw)
        if m:
            url = m.group(0)
        elif n in FALLBACK:
            url = FALLBACK[n]
        if not url:
            out.append(f"{n}\tNOT FOUND (fastoolnow search empty, no fallback)")
            continue
        raw = get(url, f"{raw_dir}/prod_{n}.html")
        sha = hashlib.sha256(raw).hexdigest()
        lines = text_of(raw)
        title = next((l for l in lines if ("Amana" in l and n in l)), "")
        specs = []
        for i, l in enumerate(lines):
            if l.startswith(SPEC_KEYS) and l not in specs:
                specs.append(l)
        # toolstoday prints the spec as one table row after the header row.
        for i, l in enumerate(lines):
            if l.startswith("DDiameter") and i + 1 < len(lines):
                specs.append("toolstoday spec header: " + l)
                specs.append("toolstoday spec row: " + lines[i + 1])
                break
        out.append(f"{n}\turl={url}\tsha256={sha}")
        out.append(f"  title: {title}")
        for s in specs[:14]:
            out.append(f"  spec: {s}")
    out.append("")

open(f"{out_dir}/amana_46xxx_identity.txt", "w").write("\n".join(out) + "\n")
print("\n".join(out))
