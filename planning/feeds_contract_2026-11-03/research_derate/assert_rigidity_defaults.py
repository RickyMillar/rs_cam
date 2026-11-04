"""Assert each claim in RIGIDITY_DEFAULTS.md. Run: python3 -I assert_rigidity_defaults.py

Read-only. Reads the tracked files at the cited hash (`git show HASH:path`), so later edits do
not move a citation. Also reads the gitignored reference/ port source when present,
and the FM1 golden CSV. Each claim id matches a [Rn] tag in the report.
"""
import collections
import csv
import io
import os
import re
import subprocess
import tomllib

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../.."))
MACH = "crates/rs_cam_core/src/machine/mod.rs"
REF = "reference/shapeoko_feeds_and_speeds/src/machine_profile.rs"
FM1 = "crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs"
CSV = "crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv"

HASH = "79649cc6c"  # the hash every path:line citation in RIGIDITY_DEFAULTS.md uses

CLAIMS = []


def claim(cid):
    def deco(fn):
        CLAIMS.append((cid, fn))
        return fn
    return deco


def rd(rel):
    if rel.startswith("reference/"):  # gitignored: the working copy only
        return open(os.path.join(ROOT, rel), encoding="utf-8").read()
    r = subprocess.run(["git", "-C", ROOT, "show", f"{HASH}:{rel}"], capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    return r.stdout


def line(rel, n):
    return rd(rel).split("\n")[n - 1]


def at(rel, n, pat):
    assert re.search(pat, line(rel, n)), f"{rel}:{n} does not match {pat!r}: {line(rel, n)!r}"


def block_values(text, keys):
    return tuple(float(re.search(rf"\b{k}:\s*([0-9.]+)", text).group(1)) for k in keys)


KEYS = ("doc_roughing_factor", "doc_finishing_factor", "woc_roughing_factor",
        "woc_roughing_max_mm", "woc_finishing_mm", "adaptive_doc_factor", "adaptive_woc_factor")
SHAPEOKO_SET = (0.25, 0.10, 0.80, 6.35, 0.635, 2.0, 0.25)
GENERIC_SET = (0.20, 0.08, 0.70, 5.0, 0.50, 1.5, 0.20)


@claim("R1")
def _():  # the two value sets and where they live
    s = rd(MACH)
    at(MACH, 166, r"^impl Default for RigidityProfile")
    d = s[s.index("impl Default for RigidityProfile"):][:600]
    assert block_values(d, KEYS) == SHAPEOKO_SET
    at(MACH, 675, r"pub fn generic_wood_router")
    at(MACH, 687, r"rigidity: RigidityProfile \{")
    g = s[s.index("pub fn generic_wood_router"):][:1200]
    assert block_values(g, KEYS) == GENERIC_SET
    at(MACH, 579, r"^impl Default for MachineProfile")
    at(MACH, 581, r"Self::generic_wood_router\(\)")
    at(MACH, 725, r"rigidity: RigidityProfile::default\(\)")
    at(MACH, 748, r"rigidity: RigidityProfile::default\(\)")
    at(MACH, 706, r"pub fn shapeoko_vfd")
    at(MACH, 733, r"pub fn shapeoko_makita")


@claim("R2")
def _():  # B03 is false: no serde default on `rigidity`; line 254 belongs to max_cutting_feed_mm_min
    at(MACH, 59, r"#\[derive\(Debug, Clone, Copy, Serialize, Deserialize\)\]")
    at(MACH, 60, r"#\[serde\(deny_unknown_fields\)\]")
    at(MACH, 61, r"pub struct RigidityProfile")
    at(MACH, 254, r"#\[serde\(default\)\]")
    at(MACH, 255, r"pub max_cutting_feed_mm_min: Option<f64>")
    at(MACH, 256, r"pub max_shank_mm: f64")
    at(MACH, 257, r"^\s*pub rigidity: RigidityProfile,$")
    # No `serde(default` between the struct header and the rigidity field except line 254.
    lines = rd(MACH).split("\n")
    hits = [i + 1 for i in range(232, 257) if "serde(default" in lines[i]]
    assert hits == [254], hits


@claim("R3")
def _():  # a project with no [job.machine] table gets MachineProfile::default(); the library loader has no fill-in
    pf = "crates/rs_cam_core/src/session/project_file.rs"
    at(pf, 781, r"#\[serde\(default\)\]")
    at(pf, 782, r"pub machine: crate::machine::MachineProfile")
    ml = "crates/rs_cam_core/src/io/machine_library.rs"
    at(ml, 90, r"pub fn load_from")
    at(ml, 99, r"toml::from_str\(&text\)")


@claim("R4")
def _():  # doors
    at("crates/rs_cam_core/src/session/mod.rs", 1984, r"machine: crate::machine::MachineProfile::default\(\)")
    at("crates/rs_cam_cli/src/job.rs", 1241, r"ProjectSession::new_empty\(\)")
    at("crates/rs_cam_viz/src/ui/properties/machine_panel.rs", 124, r"MachineProfile::presets\(\)")
    at("crates/rs_cam_viz/src/ui/properties/machine_panel.rs", 49, r"machine_library::load\(name\)")
    at("crates/rs_cam_viz/src/ui/machine_library_modal.rs", 154, r"machine_library::load\(&name\)")
    at("crates/rs_cam_viz/src/mcp_server.rs", 2029, r'name = "load_machine_from_library"')


@claim("R5")
def _():  # FM1 walks MachineProfile::default() = the generic set
    at(FM1, 1759, r"let machine = MachineProfile::default\(\);")
    at(FM1, 1783, r"let machine = MachineProfile::default\(\);")
    at(FM1, 676, r"machine\.rigidity\.depth_cap_mm")


@claim("R6")
def _():  # both sets entered in one commit, 172d6de60, 2026-03-20
    out = subprocess.run(["git", "-C", ROOT, "log", "--reverse", "--format=%h %ad %s", "--date=short",
                          "-S", "adaptive_doc_factor: 1.5"], capture_output=True, text=True).stdout
    assert out.splitlines()[0].startswith("172d6de60 2026-03-20 Add feeds & speeds core library"), out
    out = subprocess.run(["git", "-C", ROOT, "log", "--reverse", "--format=%h", "-S",
                          "adaptive_doc_factor: 2.0"], capture_output=True, text=True).stdout
    assert out.splitlines()[0] == "172d6de60", out
    msg = subprocess.run(["git", "-C", ROOT, "show", "-s", "--format=%B", "172d6de60"],
                         capture_output=True, text=True).stdout
    assert "ported from reference" in msg and "shapeoko_feeds_and_speeds" in msg


@claim("R7")
def _():  # the gitignored port source holds both sets; generic drifted on doc_finishing (0.05 -> 0.08)
    p = os.path.join(ROOT, REF)
    if not os.path.exists(p):
        print("SKIP R7: reference/ is gitignored and not on this machine")
        return
    at(REF, 131, r"pub fn shapeoko_vfd_1500w_balanced")
    at(REF, 426, r"// Start with conservative defaults")
    s = rd(REF)
    bal = s[s.index("pub fn shapeoko_vfd_1500w_balanced"):]
    bal = bal[bal.index("doc_profile: DocProfile"):][:900]
    con = s[s.index("// Start with conservative defaults"):]
    con = con[con.index("doc_profile: DocProfile"):][:900]

    def ref_set(b):
        k = lambda name: float(re.search(rf"\b{name}:\s*([0-9.]+)", b).group(1))
        w = b[b.index("woc_profile"):]
        kw = lambda name: float(re.search(rf"\b{name}:\s*([0-9.]+)", w).group(1))
        return (k("wide_shallow_roughing_factor"), k("wide_shallow_finishing_factor"),
                kw("wide_shallow_roughing_factor"), kw("wide_shallow_roughing_max"),
                kw("wide_shallow_finishing"), k("deep_narrow_roughing_factor"),
                kw("deep_narrow_roughing_factor"))
    assert ref_set(bal) == SHAPEOKO_SET, ref_set(bal)
    c = ref_set(con)
    assert c == (0.2, 0.05, 0.7, 5.0, 0.5, 1.5, 0.2), c
    diffs = [KEYS[i] for i in range(7) if c[i] != GENERIC_SET[i]]
    assert diffs == ["doc_finishing_factor"], diffs
    assert os.path.exists(os.path.join(ROOT, ".gitignore"))
    assert re.search(r"^/reference/$", rd(".gitignore"), re.M)


@claim("R8")
def _():  # tracked TOML files: which set each states
    files = subprocess.run(["git", "-C", ROOT, "ls-files", "*.toml"], capture_output=True, text=True).stdout.split()
    tally = collections.Counter()
    for f in files:
        t = rd(f)
        if "rigidity" not in t:
            continue
        try:
            d = tomllib.loads(t)
        except Exception:
            continue

        def find(o):
            if isinstance(o, dict):
                if isinstance(o.get("rigidity"), dict):
                    return o["rigidity"]
                for v in o.values():
                    r = find(v)
                    if r:
                        return r
            return None
        r = find(d)
        if r:
            tally[(r["doc_roughing_factor"], r["adaptive_doc_factor"], r["adaptive_woc_factor"],
                   r["woc_roughing_factor"])] += 1
    assert tally[(0.2, 1.5, 0.2, 0.7)] == 114, tally
    assert tally[(0.25, 2.0, 0.2, 0.4)] == 14, tally
    assert tally[(0.25, 2.0, 0.25, 0.8)] == 4, tally
    assert sum(tally.values()) == 132, tally


@claim("R9")
def _():  # FM1 golden CSV: cap columns and adaptive stepover that read the generic set
    rows = list(csv.DictReader(io.StringIO(rd(CSV))))
    assert len(rows) == 1000
    c = collections.Counter((r["cap_family"], r["cap_factor"]) for r in rows if r["cap_factor"])
    assert c == {("Adaptive", "1.5000"): 80, ("Contour", "0.2000"): 40, ("Pocket", "0.2000"): 160}, c
    ok = [r for r in rows if r["status"] == "ok"]
    assert sum(1 for r in ok if r["cap_factor"]) == 202
    ad = [r for r in ok if r["operation"] in ("Adaptive", "Adaptive3d")]
    assert len(ad) == 60
    at02 = [r for r in ad if abs(float(r["stepover_mm"]) / float(r["diameter_mm"]) - 0.2) < 5e-4]
    assert len(at02) == 32, len(at02)

    def f(s):
        try:
            return float(s)
        except ValueError:
            return None
    clamped = [r for r in ok if f(r["cap_mm"]) and f(r["depth_per_pass_mm"])
               and abs(f(r["depth_per_pass_mm"]) - f(r["cap_mm"])) < 1e-3]
    assert clamped == []


@claim("R10")
def _():  # the hard-coded 0.20 in the stale-default fix, and the research_ae finding on "20 %"
    v = "crates/rs_cam_core/src/compute/validate.rs"
    at(v, 284, r"let new_value = 0\.20 \* d;")
    r = "planning/feeds_contract_2026-11-03/research_ae/REPORT.md"
    at(r, 14, r'"20 %": no repo source prints 20 % as an optimal load for wood')
    s = "planning/feeds_contract_2026-11-03/STATUS.md"
    assert "states that 0.2 is the Fusion 360 default (an operator statement; find a printed source)" in rd(s)


@claim("R11")
def _():  # the operator machine file (outside the repo)
    p = os.path.expanduser("~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml")
    if not os.path.exists(p):
        print("SKIP R11: the operator machine file is not on this machine")
        return
    d = tomllib.load(open(p, "rb"))["rigidity"]
    assert (d["doc_roughing_factor"], d["adaptive_doc_factor"], d["adaptive_woc_factor"],
            d["woc_roughing_factor"]) == (0.25, 2.0, 0.2, 0.4)


if __name__ == "__main__":
    for cid, fn in CLAIMS:
        fn()
        print("OK", cid)
    print("ALL RIGIDITY CLAIMS HOLD")
