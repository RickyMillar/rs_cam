#!/usr/bin/env python3
"""G7 density fetch checker (Radiata pine, Jarrah, Ipe; plan B6 group G7).

Re-reads each stored source text, confirms `row_verbatim` is present in it,
parses the printed specific gravity, and recomputes `gm12` / `rho12_kg_m3`.

For an FPL Table 5-5a row, it applies FPL Ch.4 Eq. (4-11),
Gx = Gb / [1 - 0.265 Gb (1 - x/MCfs)] with x=12 and MCfs=30 (FPL's stated
average fiber saturation point), and checks the equation string itself is
present verbatim in the Ch.4 excerpt before trusting it -- this script
does not carry the equation from memory independent of the fetched text.

For a Wood Database row, it recomputes `rho12 = SG_12%MC * 1000` (that
source's own tooltip states its second SG number already has both mass
and volume at 12% MC, so no further factor applies) and checks it against
the declared value, and also checks the declared `cross_check_wdb` ratio
on the matching FPL row.

Usage: python3 scripts/g7_density_check.py   (from planning/extrapolation_2026-09-24)
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
G7 = os.path.join(HERE, "..", "fetch", "G7")
TOLERANCE_KG_M3 = 1.0
TOLERANCE_GM12 = 1e-4
TOLERANCE_RATIO = 1e-3

CH4_EXCERPT = "sources/fpl_gtr190_ch4_sg_conversion_excerpt.txt"
CH4_EQUATION_STRING = "Gx = Gb / [1 - 0.265 Gb (1 - x/MCfs)]"
MCFS_DEFAULT = 30.0
TARGET_MC = 12.0


def eq_4_11(gb, mcfs=MCFS_DEFAULT, x=TARGET_MC):
    """FPL Ch.4 Eq. (4-11)."""
    a = (mcfs - x) / mcfs
    return gb / (1 - 0.265 * gb * a)


def norm_ws(s):
    return " ".join(s.split())


def load_rows():
    with open(os.path.join(G7, "density_rows.json"), encoding="utf-8") as f:
        return json.load(f)["observations"]


def read_stored_text(rel_path):
    path = os.path.join(G7, rel_path)
    with open(path, encoding="utf-8") as f:
        return f.read()


def parse_specific_gravity(row):
    """Recover the printed SG from row_verbatim, cross-checked against
    printed_value. FPL rows print one SG per line (the species' Green
    row); Wood Database rows print 'basic, 12% MC' (two numbers) and we
    want the second (12% MC) one, which is what printed_value holds."""
    verbatim = row["row_verbatim"]
    source_id = row["source_id"]
    if source_id.startswith("g7_fpl_"):
        # e.g. "Ipe (Tabebuia spp.,                Green    0.92    155,800 ..."
        tokens = verbatim.split()
        green_idx = tokens.index("Green")
        return float(tokens[green_idx + 1])
    if source_id.startswith("g7_wood_database_"):
        # e.g. "Specific Gravity (Basic, 12% MC): .41, .51"
        after_colon = verbatim.split(":", 1)[1]
        fields = [f.strip() for f in after_colon.split(",")]
        return float(fields[1])
    raise ValueError(f"unrecognised source_id for parsing: {source_id}")


def recompute_gm12_and_rho12(row, printed_sg, failures):
    """Returns (gm12, rho12) for a row, or (None, None) if the row's own
    quantity is not a 12%-MC-basis SG (nothing left to derive)."""
    source_id = row["source_id"]
    if source_id.startswith("g7_fpl_"):
        gm12 = eq_4_11(printed_sg)
        rho12 = gm12 * 1000.0 * 1.12
        return gm12, rho12
    if source_id.startswith("g7_wood_database_"):
        # Source states outright: second SG number already has both mass
        # and volume at 12% MC, so rho12 = SG * density of water.
        return None, printed_sg * 1000.0
    raise ValueError(f"unrecognised source_id for recompute: {source_id}")


def check_ch4_equation_on_file(failures):
    """The FPL row derivation depends on Eq. (4-11); confirm the equation
    string is actually present in the fetched Ch.4 excerpt before this
    script trusts it, rather than carrying the equation from memory
    independent of the source."""
    text = read_stored_text(CH4_EXCERPT)
    if norm_ws(CH4_EQUATION_STRING) not in norm_ws(text):
        failures.append(
            f"Eq. (4-11) string {CH4_EQUATION_STRING!r} not found verbatim in {CH4_EXCERPT}"
        )
        return False
    return True


def main():
    rows = load_rows()
    failures = []

    check_ch4_equation_on_file(failures)

    print(f"{'species':45s} {'source_id':32s} {'printed_SG':>10s} {'gm12':>10s} {'rho12 (recomputed)':>20s} {'rho12 (declared)':>18s}  status")
    for row in rows:
        species = row["species"]
        source_id = row["source_id"]
        stored_text = read_stored_text(row["stored_text"])
        verbatim = row["row_verbatim"]

        if norm_ws(verbatim) not in norm_ws(stored_text):
            failures.append(f"{source_id}: row_verbatim not found in {row['stored_text']}")
            status = "FAIL (verbatim not found)"
            print(f"{species:45s} {source_id:32s} {'--':>10s} {'--':>10s} {'--':>20s} {'--':>18s}  {status}")
            continue

        printed_sg = parse_specific_gravity(row)
        if abs(printed_sg - row["printed_value"]) > 1e-9:
            failures.append(
                f"{source_id}: parsed SG {printed_sg} != declared printed_value {row['printed_value']}"
            )

        gm12, recomputed = recompute_gm12_and_rho12(row, printed_sg, failures)
        declared = row["rho12_kg_m3"]
        declared_gm12 = row.get("gm12")

        status = "OK"
        if gm12 is not None:
            if declared_gm12 is None:
                failures.append(f"{source_id}: recomputed gm12={gm12:.5f} but declared gm12 is missing")
                status = "FAIL (gm12 missing)"
            elif abs(gm12 - declared_gm12) > TOLERANCE_GM12:
                failures.append(
                    f"{source_id}: recomputed gm12={gm12:.5f} != declared {declared_gm12}"
                )
                status = "FAIL (gm12 mismatch)"

        if declared is None:
            failures.append(f"{source_id}: recomputed rho12={recomputed:.1f} but declared rho12 is null")
            status = "FAIL (declared null)"
        elif abs(recomputed - declared) > TOLERANCE_KG_M3:
            failures.append(f"{source_id}: recomputed rho12={recomputed:.1f} != declared {declared}")
            status = "FAIL (rho12 mismatch)"

        gm12_str = f"{gm12:.5f}" if gm12 is not None else "--"
        recomputed_str = f"{recomputed:.1f}"
        declared_str = f"{declared:.1f}" if declared is not None else "null"
        print(f"{species:45s} {source_id:32s} {printed_sg:>10.3f} {gm12_str:>10s} {recomputed_str:>20s} {declared_str:>18s}  {status}")

        # Cross-check the declared WDB ratio on FPL rows that carry one.
        cc = row.get("cross_check_wdb")
        if cc is not None:
            recomputed_ratio = recomputed / cc["wdb_rho12_kg_m3"]
            if abs(recomputed_ratio - cc["ratio_fpl_over_wdb"]) > TOLERANCE_RATIO:
                failures.append(
                    f"{source_id}: recomputed ratio_fpl_over_wdb={recomputed_ratio:.4f} "
                    f"!= declared {cc['ratio_fpl_over_wdb']}"
                )
                status = "FAIL (ratio mismatch)"
            print(f"    cross-check vs Wood Database: FPL {recomputed:.1f} / WDB "
                  f"{cc['wdb_rho12_kg_m3']:.1f} = {recomputed_ratio:.4f} "
                  f"(declared {cc['ratio_fpl_over_wdb']:.4f})")

    print()
    if failures:
        print(f"{len(failures)} FAILURE(S):")
        for f in failures:
            print(f"  - {f}")
        return 1

    print("All rows check out: every row_verbatim is present in its stored")
    print("text, every parsed SG matches printed_value, Eq. (4-11) is on")
    print("file verbatim, and every gm12 / rho12 / WDB ratio recomputes to")
    print("the declared value.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
