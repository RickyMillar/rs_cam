"""G6: list the red-printed worked-example values on the Leitz Lexicon
drilling diagrams (feed speed vf against spindle RPM n).

The diagrams print one worked example per page in red text: an RPM on
the n axis and a feed speed on the vf axis. This script reads the text
spans and their colour from the PDF. It does not read the plotted bands
(those are graphics, not text).

Usage: python g6_leitz_red_points.py <leitz_drilling.pdf>
Needs PyMuPDF (pip install pymupdf).
"""
import sys

import pymupdf


def is_red(color: int) -> bool:
    r, g, b = (color >> 16) & 255, (color >> 8) & 255, color & 255
    return r > 150 and g < 90 and b < 90


def main(path: str) -> None:
    doc = pymupdf.open(path)
    for pno, page in enumerate(doc, start=1):
        text = page.get_text()
        if "depending on the spindle" not in text:
            continue
        reds = []
        for block in page.get_text("dict")["blocks"]:
            for line in block.get("lines", []):
                for span in line["spans"]:
                    t = span["text"].strip()
                    if t and is_red(span["color"]):
                        x0, y0 = span["bbox"][0], span["bbox"][1]
                        reds.append((round(x0), round(y0), t))
        print(f"page {pno}: red spans {reds}")


if __name__ == "__main__":
    main(sys.argv[1])
