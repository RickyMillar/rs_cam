"""One assert per claim id on index.html. Reads results.json. Exits non-zero if a claim fails.
Run: python3 -I assert_claims.py   (after python3 -I model.py and python3 -I build_page.py)
"""
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
R = json.load(open(os.path.join(HERE, "results.json")))
HTML = open(os.path.join(HERE, "index.html")).read()
WOOD = {"hardwood", "softwood", "mdf", "plywood_hardwood", "plywood_softwood", "hdf", "particleboard"}
CANDS = ["I1", "I2", "I3", "I4", "I3v", "I5", "C1", "C2"]
F = R["fractions"]
W = [c for c in R["cases"] if c["group"] == "W"]
D = [c for c in R["cases"] if c["group"] == "D"]
H = R["hand_check"]


def case(label):
    return next(c for c in W if c["label"] == label)


def near(a, b, tol):
    return abs(a - b) <= tol


CLAIMS = {}


def claim(cid):
    def deco(f):
        CLAIMS[cid] = f
        return f
    return deco


@claim("C01")
def _():  # verdict FAIL: no candidate meets 80 % in both groups
    assert R["verdict"] == "FAIL" and R["winners"] == []
    for k in CANDS:
        assert not (F["W"][k]["frac"] >= 0.8 and F["D"][k]["frac"] >= 0.8)


@claim("C02")
def _():  # 7 W cases (6 force-testable), 466 D cases from 233 rows
    assert len(W) == 7 and F["W"]["I3"]["n"] == 6
    assert len(D) == 466 and len({c["id"] for c in D}) == 233


@claim("C03")
def _():  # 18 flat rows carry a printed width, 20 a repo-authored width, 286 flat rows in all
    assert R["n_printed_ae_rows"] == 18 and R["n_repo_ae_rows"] == 20 and R["n_flat_rows"] == 286


@claim("C04")
def _():  # best W: I4 and C2 at 4 of 6 (67 %)
    best = max(F["W"][k]["frac"] for k in CANDS)
    assert near(best, 4 / 6, 1e-9)
    assert {k for k in CANDS if near(F["W"][k]["frac"], best, 1e-9)} == {"I4", "C2"}
    assert F["W"]["I4"]["passed"] == 4 and F["W"]["I4"]["n"] == 6


@claim("C05")
def _():  # best D: I3v at 34 of 466 (7.3 %)
    best = max(CANDS, key=lambda k: F["D"][k]["frac"])
    assert best == "I3v" and F["D"]["I3v"]["passed"] == 34 and near(F["D"]["I3v"]["frac"], 0.073, 0.0005)


@claim("C06")
def _():  # on the 418 wood depth cases, every force candidate except I5 passes 0; I5 16; I1 1
    wood = [c for c in D if c["material"] in WOOD]
    assert len(wood) == 418
    for k in ("I3", "I4", "I3v", "C1", "C2"):
        assert sum(c["passes"][k] for c in wood) == 0, k
    assert sum(c["passes"]["I5"] for c in wood) == 16
    assert sum(c["passes"]["I1"] for c in wood) == 1


@claim("C07")
def _():  # lead's hand check: one-tooth cut 80.0 % at fz 0.131 in oak: confirmed
    assert near(H["one_tooth_cut"], 0.800, 0.0005) and H["confirmed"]


@claim("C08")
def _():  # engaged-edge cuts: peak 78.2 %, mean 97.1 %, vector 61.1 %, deflection 11.2 %
    assert near(H["cut"]["I3"], 0.782, 0.0005)
    assert near(H["cut"]["I4"], 0.971, 0.0005)
    assert near(H["cut"]["I3v"], 0.611, 0.0005)
    assert near(H["cut"]["I5"], 0.112, 0.0005)


@claim("C09")
def _():  # vendor cut 25 %, pure shear 50 %
    assert H["vendor_cut"] == 0.25 and H["pure_shear_cut"] == 0.5


@claim("C10")
def _():  # 209 wood rows at 2xD: I3 cut median 74.6 %, min 48.1 %, max 100 %
    s = H["rows_cut_2xD"]["I3"]
    assert s["n"] == 209 and near(s["median"], 0.746, 0.0005) and near(s["min"], 0.481, 0.0005) and s["max"] == 1.0
    assert near(H["rows_cut_2xD"]["I4"]["median"], 0.926, 0.0005)


@claim("C11")
def _():  # the vendor step lets the peak force rise x1.51 median at 2xD (1.26-1.83) and x1.72 at 3xD
    v2, v3 = H["vendor_force_ratio"]["2"], H["vendor_force_ratio"]["3"]
    assert near(v2["median"], 1.51, 0.005) and near(v2["min"], 1.26, 0.005) and near(v2["max"], 1.83, 0.005)
    assert near(v3["median"], 1.72, 0.005)


@claim("C12")
def _():  # chip thinning alone predicts a HEM chip of 0.063 mm; Garr prints 0.12-0.18
    c = case("Garr A3 slot -> hem-profile")
    assert near(c["pred"]["I1"], 0.063, 0.0005) and c["tgt_band"] == [0.12, 0.18] and not c["passes"]["I1"]


@claim("C13")
def _():  # Garr finish: I1 predicts 0.192 mm, Garr prints 0.06 (3.2x)
    c = case("Garr A3 slot -> finish")
    assert near(c["pred"]["I1"], 0.192, 0.0005) and c["tgt_band"] == [0.06, 0.06]
    assert near(c["pred"]["I1"] / 0.06, 3.2, 0.05)


@claim("C14")
def _():  # Helical: I1 0.252 (+7.3 %), I3 0.2245 (-4.5 %), printed 0.235
    c = case("Helical traditional -> HEM")
    assert near(c["pred"]["I1"], 0.252, 0.0005) and near(c["pred"]["I1"] / 0.235 - 1, 0.073, 0.001)
    assert near(c["pred"]["I3"], 0.2245, 0.0005) and near(c["pred"]["I3"] / 0.235 - 1, -0.045, 0.001)
    assert c["passes"]["I3"] and not c["passes"]["I1"]


@claim("C15")
def _():  # no candidate passes in any of the 13 sensitivity runs
    assert len(R["sensitivity"]) == 13 and all(s["winners"] == [] for s in R["sensitivity"])


@claim("C16")
def _():  # 84 rows print a feed; feed/(rpm z) is within 2.8 % of the stored chip in all of them
    assert R["fc0"]["n"] == 84 and R["fc0"]["max_rel_dev"] <= 0.028


@claim("C17")
def _():  # c wood 0.0785 mm, c MDF 0.1069 mm
    assert near(R["c_values"]["wood"], 0.0785, 0.00005) and near(R["c_values"]["mdf"], 0.1069, 0.00005)


@claim("C18")
def _():  # post hoc PH2: chip proportional to 1 / time in cut matches 4 of 7 W cases
    ph = R["post_hoc"]["PH2_inverse_time_in_cut"]
    assert len(ph) == 7 and sum(bool(p["passes"]) for p in ph) == 4
    failed = {p["label"] for p in ph if not p["passes"]}
    assert failed == {"Garr 142M slot -> profile", "Garr A3 slot -> finish", "Helical traditional -> HEM"}


@claim("C19")
def _():  # post hoc PH1: I5 gives the 25 % step at stickout 3.5 to 4 x D on six hardwood rows
    ph = R["post_hoc"]["PH1_stickout_for_25pct"]
    assert len(ph) == 6 and all(3.5 <= p["l_over_d"] <= 4.0 for p in ph)


@claim("C20")
def _():  # Garr slot -> profile: I1 gives the slot chip 0.06, the band floor; strict ratio fails (printed 1.5)
    for s in ("242M", "842M", "A3"):
        c = case(f"Garr {s} slot -> profile")
        assert near(c["pred"]["I1"], 0.06, 1e-9) and c["tgt_band"][0] == 0.06 and c["passes"]["I1"]
        assert near(c["printed_ratio"], 1.5, 1e-9) and not c["ratio_ok"]["I1"]


@claim("C21")
def _():  # strict ratio test: no candidate exceeds 20 % of D cases or 29 % of W cases
    FR = R["fractions_ratio"]
    assert max(FR["D"][k]["frac"] for k in CANDS) < 0.20
    assert max(FR["W"][k]["frac"] for k in CANDS) <= 2 / 7 + 1e-9


def main():
    ids_on_page = set(re.findall(r'data-claim="(C\d\d)"', HTML))
    fails = []
    if ids_on_page != set(CLAIMS):
        fails.append(f"claim ids differ: page-only {sorted(ids_on_page - set(CLAIMS))}, "
                     f"assert-only {sorted(set(CLAIMS) - ids_on_page)}")
    for cid, f in sorted(CLAIMS.items()):
        try:
            f()
            print(f"{cid} ok")
        except AssertionError as e:
            fails.append(f"{cid} FAILED {e}")
            print(f"{cid} FAILED {e}")
    if fails:
        print("\n".join(fails))
        sys.exit(1)
    print(f"ALL {len(CLAIMS)} CLAIMS HOLD")


if __name__ == "__main__":
    main()
