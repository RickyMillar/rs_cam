#!/usr/bin/env python3
"""Check every row of onsrud_vbit_37.json against the stored sheet text.

Run: python3 planning/extrapolation_2026-09-24/scripts/check_onsrud_vbit_37.py

For each row the script reads the stored text that the manifest names
(`stored_text` of the row's `source_id`) and asserts:

- the line that the row's notes cite starts with the cited series label;
- the cited band sits in the cited column (the nearest header column by
  character position, the g2_column_map.py method), and the line prints it;
- chipload_min / chipload_max = the printed band x 25.4, to 4 decimals;
- diameter_mm = the column heading x 25.4, to 4 decimals; the 37-00/37-20
  '1/4' column is the shank, so that row has no diameter;
- ap_rule names the Cut column that the line prints;
- source_page names the page number that the sheet footer prints, the
  series, the column and the band;
- included_angle_deg and flute_count match the PCT-19 catalogue text
  (sources/onsrud_pct19_catalog.txt) for the series and size.

The script reads no PDF and writes nothing. It exits non-zero on the first
row that fails and prints one line per row that passes.
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
LUT = ROOT / "crates" / "rs_cam_core" / "data" / "vendor_lut"
OBS = LUT / "observations" / "onsrud_vbit_37.json"
MANIFEST = LUT / "source_manifest.json"
CATALOG = LUT / "sources" / "onsrud_pct19_catalog.txt"
IN_MM = 25.4

NOTE = re.compile(
    r"line (\d+), the '([^']+)' row, prints (\.\d{3} ?-\s?\.\d{3}) under the '([^']+)' "
    r"column of the header on line (\d+)"
)
BAND = re.compile(r"\.\d{3} ?-\s?\.\d{3}\*{0,2}")
HEADER_COL = re.compile(r"(\d+(?:[ -]\d+/\d+|/\d+)?)")


def frac(s):
    """'3/16' -> 0.1875, '1 1/2' or '1-1/2' -> 1.5, '1' -> 1.0 (inch)."""
    total = 0.0
    for p in s.strip().replace("-", " ").split():
        if "/" in p:
            a, b = p.split("/")
            total += float(a) / float(b)
        else:
            total += float(p)
    return total


def band_mm(text):
    lo, hi = text.rstrip("*").replace(" ", "").split("-")
    return round(float(lo) * IN_MM, 4), round(float(hi) * IN_MM, 4)


def columns(header):
    cols = []
    for m in HEADER_COL.finditer(header):
        if m.start() < 15:
            continue
        cols.append((m.group(1), (m.start() + m.end()) / 2))
    return cols


def catalog_angle_flutes(series, size_in):
    """The included angle and flute count PCT-19 prints for the series."""
    cat = CATALOG.read_text(encoding="utf-8")
    if series in ("37-00", "37-20"):
        # PCT-19 page 20 rows: part, tip, angle, shank, ..., flutes. Every
        # part of the series must print one angle and one flute count.
        lo = 0 if series == "37-00" else 20
        parts = re.compile(r"\s37-(\d{2})\s+0\.\d{3}\s+(\d+)\s+1/4\s+\S+\s+(\d+)\s*$")
        seen = set()
        for ln in cat.split("\n"):
            pm = parts.search(ln)
            if pm and lo < int(pm.group(1)) < lo + 20:
                seen.add((float(pm.group(2)), int(pm.group(3))))
        assert len(seen) == 1, f"{series}: PCT-19 prints {seen}"
        return seen.pop()
    if series in ("37-50", "37-60"):
        assert f"{series} Series Two Flute - V Bottom" in cat, series
        assert "Designed for V grooving or beveling 90°." in cat
        return 90.0, 2
    if series == "37-80":
        part = {1.0: ("37-82", "1 "), 1.5: ("37-87", "1-1/2")}[size_in]
        line = next(ln for ln in cat.split("\n") if f" {part[0]} " in ln)
        nums = line.split(part[0], 1)[1].split()
        # PCT-19 page 22 row: part, CED, ..., angle, flutes (last two cells).
        angle = float(nums[-2].rstrip("º°"))
        return angle, int(nums[-1])
    raise AssertionError(f"no catalogue rule for {series}")


def main():
    manifest = json.loads(MANIFEST.read_text())
    sources = manifest["sources"] if isinstance(manifest, dict) else manifest
    stored = {s["source_id"]: s.get("stored_text") for s in sources}
    rows = json.loads(OBS.read_text())["observations"]
    ids = [r["observation_id"] for r in rows]
    assert len(ids) == len(set(ids)), "duplicate observation_id"
    for r in rows:
        oid = r["observation_id"]
        rel = stored.get(r["source_id"])
        assert rel, f"{oid}: the manifest has no stored_text for {r['source_id']}"
        assert r["notes"].find(rel) >= 0, f"{oid}: the notes do not cite {rel}"
        lines = (LUT / rel).read_text(encoding="utf-8").split("\n")
        m = NOTE.search(r["notes"])
        assert m, f"{oid}: the notes cite no line / column"
        row_ln, series_label, band_text, col_name, hdr_ln = m.groups()
        row = lines[int(row_ln) - 1].expandtabs(8)
        hdr = lines[int(hdr_ln) - 1].expandtabs(8)
        assert row.strip().startswith(series_label), f"{oid}: line {row_ln} is {row[:20]!r}"
        assert hdr.strip().startswith("Series"), f"{oid}: line {hdr_ln} is not the header"
        cols = columns(hdr)
        found = None
        for b in BAND.finditer(row):
            c = (b.start() + b.end()) / 2
            name, _ = min(cols, key=lambda k: abs(k[1] - c))
            if name == col_name:
                found = b.group(0)
        assert found, f"{oid}: line {row_ln} prints no band under '{col_name}'"
        assert found.rstrip("*").replace(" ", "") == band_text.replace(" ", ""), (
            f"{oid}: '{col_name}' prints {found}, the notes say {band_text}"
        )
        lo, hi = band_mm(found)
        assert r["chipload_min_mm_tooth"] == lo, f"{oid}: min {r['chipload_min_mm_tooth']} != {lo}"
        assert r["chipload_max_mm_tooth"] == hi, f"{oid}: max {r['chipload_max_mm_tooth']} != {hi}"
        # Cut column: the text between the series label and the first band.
        cut = row.split(series_label, 1)[1]
        cut = BAND.split(cut)[0].strip()
        assert r["ap_rule"] == f"printed Cut column: '{cut}'", f"{oid}: ap_rule vs {cut!r}"
        series = oid.split("-37-")[1].split("-")[0]
        series = f"37-{series}"
        size_in = frac(col_name)
        if series_label == "37-00/37-20":
            assert col_name == "1/4" and "diameter_mm" not in r, f"{oid}: shank column has no diameter"
        else:
            want = round(size_in * IN_MM, 4)
            assert r["diameter_mm"] == want, f"{oid}: diameter {r['diameter_mm']} != {want}"
        page = next(
            (p for ln in lines[-12:] for p in re.findall(r"\b(11[3-9])\b", ln) if "onsrud.com" in ln),
            None,
        )
        assert page, f"{oid}: no page number in the stored text footer"
        sp = r["source_page"]
        assert f"page {page}" in sp, f"{oid}: source_page {sp!r} vs printed page {page}"
        assert series_label in sp and band_text.replace(" ", "") in sp.replace(" ", ""), oid
        assert f"{col_name} in" in sp, f"{oid}: source_page names no '{col_name} in' column"
        angle, flutes = catalog_angle_flutes(series, size_in)
        assert r["included_angle_deg"] == angle, f"{oid}: angle {r['included_angle_deg']} != {angle}"
        assert r["flute_count"] == flutes, f"{oid}: flutes {r['flute_count']} != {flutes}"
        print(f"ok {oid}: {rel}:{row_ln} '{col_name}' {found} -> {lo}-{hi} mm, page {page}")
    print(f"{len(rows)} rows checked against the stored text")
    return 0


if __name__ == "__main__":
    sys.exit(main())
