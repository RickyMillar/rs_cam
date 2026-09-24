#!/usr/bin/env python3
"""Convert a Europe PMC JATS full-text XML file to plain text.

The script keeps the text as the article prints it. It writes one line per
paragraph, title and caption, and one line per table row with the cells
joined by " | ". It does not change any number.

Usage: jats2txt.py <in.xml> <out.txt>
"""
import sys
import xml.etree.ElementTree as ET


def text_of(el):
    return " ".join("".join(el.itertext()).split())


def main():
    src, dst = sys.argv[1], sys.argv[2]
    root = ET.parse(src).getroot()
    out = []
    for el in root.iter():
        tag = el.tag.split("}")[-1]
        if tag in ("article-title", "title"):
            out.append("\n## " + text_of(el))
        elif tag == "p":
            # Skip paragraphs inside table cells; the row handles them.
            out.append(text_of(el))
        elif tag == "caption":
            out.append("[caption] " + text_of(el))
        elif tag == "tr":
            cells = [text_of(c) for c in el if c.tag.split("}")[-1] in ("td", "th")]
            out.append("[row] " + " | ".join(cells))
        elif tag == "disp-formula":
            out.append("[formula] " + text_of(el))
    with open(dst, "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
