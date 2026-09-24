#!/usr/bin/env python3
"""G10 fetch, part `wood`: write the text copies of the stored raw files.

A PDF gets `pdftotext -layout` under a three-line header (url, access date,
pdf sha256). An HTML page goes through g5_html_to_text.py; this script then
sets the access date line to the real access date. Prints the sha256 of each
raw file and each text file.

Usage: python3 g10_wood_store.py
"""
import hashlib
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
G10 = os.path.normpath(os.path.join(HERE, "..", "fetch", "G10"))
ACCESSED = "2026-09-25"

PDFS = {
    "wood_amana_compression_v8": "https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Compression-Spirals-v8.pdf",
    "wood_amana_spektra_plunge_v24": "https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Spektra-Spiral-Plunge-2-3-Flute-v24.pdf",
    "wood_amana_ball_nose_v7": "https://www.amanatool.com/pub/media/productattachments/Spiral-Ball-Nose-Speed-Chart-v7.pdf",
    "wood_amana_insert_vgroove_v16": "https://www.amanatool.com/pub/media/productattachments/Insert-V-Groove-Speed-Chart-v16.pdf",
    "wood_amana_spektra_engraving_v4": "https://www.amanatool.com/pub/media/productattachments/Spektra-15-30-45-120-Degree-Engraving-Speed-Chart-v4.pdf",
    "wood_amana_corner_radius_plunge": "https://toolstoday.com/content/ProductFile/Attachments/Solid-Carbide-Spiral-Plunge-w-Corner-Radius.pdf",
    "wood_vortex_catalog": "https://www.vortextool.com/media/assets/Vortex_Catalog.pdf",
    "wood_onsrud_routing_guide": "https://precisionboard.com/wp-content/uploads/2017/08/CNC-Prod-Routing-Guide-05.pdf",
    "wood_onsrud_pct19": "https://onsrud.com/images/LMT%20Onsrud%20Product%20Cutting%20Tools%20Catalog%20PCT-19.pdf",
    "wood_idc_feeds_speeds": "https://community.carbide3d.com/uploads/short-url/fwPIYiWQNjUx8eEwsA7qmYiLMxv.pdf",
    "wood_whiteside_cnc_brochure": "https://cdn.shopify.com/s/files/1/1698/7023/files/CNC_Brochure_1-14-19.pdf?20",
}
HTMLS = {
    "wood_onsrud_plastics_faq2": "https://onsrud.com/articles/Frequently-Asked-Questions-in-the-Routing-of-Plastics-2.asp",
    "wood_toolstoday_calc_video": "https://toolstoday.com/t-video-how-to-calculate-feeds-and-speeds",
    "wood_toolstoday_understanding_feeds_speeds": "https://toolstoday.com/learn/understanding-cnc-feeds-and-speeds",
    "wood_axyz_compression_bit_tip": "https://www.axyz.com/using-a-compression-bit/",
}


def sha256(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def main():
    for sid, url in PDFS.items():
        raw = os.path.join(G10, "pdf", sid + ".pdf")
        out = os.path.join(G10, "sources", sid + ".txt")
        text = subprocess.run(["pdftotext", "-layout", raw, "-"], capture_output=True, text=True, check=True).stdout
        with open(out, "w") as fh:
            fh.write(f"# source_url: {url}\n# accessed_on: {ACCESSED}\n# pdf_sha256: {sha256(raw)}\n")
            fh.write(text)
        print(sid, sha256(raw), sha256(out))
    for sid, url in HTMLS.items():
        raw = os.path.join(G10, "html", sid + ".html")
        out = os.path.join(G10, "sources", sid + ".txt")
        subprocess.run([sys.executable, os.path.join(HERE, "g5_html_to_text.py"), raw, out, url], check=True)
        body = open(out).read().replace("# accessed_on: 2026-09-24\n", f"# accessed_on: {ACCESSED}\n", 1)
        with open(out, "w") as fh:
            fh.write(body)
        print(sid, sha256(raw), sha256(out))


if __name__ == "__main__":
    main()
