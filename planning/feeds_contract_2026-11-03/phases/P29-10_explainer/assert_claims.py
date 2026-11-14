#!/usr/bin/env python3
"""Assert each claim of the P29-10 explainer page (P29-10_DESIGN.md section 8,
with the lead rulings of section 16).

The script reads only the files in data/ next to it and the page
index.html (for claim 8). It prints each claim with PASS or FAIL and the
numbers behind it, and exits non-zero when a claim is false. A claim with
zero cases prints NOT TESTED and also makes the script exit non-zero (the
P29-10 review, section 17 item 1): a claim that judged nothing proves
nothing.

data/:
  mcp_safe_envelope_operator_<idx>.json  MCP get_safe_envelope replies
  mcp_tool_load_report_operator.json      MCP get_tool_load_report reply
  cli_suggest_<project>.jsonl             CLI `suggest --json` output
  field_<project>.json                    the force planes (core door,
                                          written by the instrument
                                          p29_10_explainer_field_instrument)
                                          at the depth top of the GUI chart
  gui_deepest_sample.json                 the deepest simulated sample of
                                          each op (GUI cut force card); the
                                          instrument reads it through
                                          RS_CAM_P29_10_DEEPEST_SAMPLE
"""

import bisect
import glob
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")
GROUPS = ["body", "entry", "cleanup", "link", "drill"]
REL = 1e-9

results = []


def claim(cid, text, ok, detail, cases):
    """Record one claim. `cases` is the number of cases the claim judged;
    zero cases is NOT TESTED, which fails like a false claim."""
    tested = cases > 0
    ok = ok and tested
    results.append((cid, text, ok, detail))
    word = "PASS" if ok else ("FAIL" if tested else "NOT TESTED")
    print(f"[{word}] claim {cid}: {text} ({cases} cases)")
    for line in detail:
        print(f"         {line}")


def load(name):
    with open(os.path.join(DATA, name)) as f:
        return json.load(f)


def mcp_envelopes():
    out = {}
    for path in sorted(glob.glob(os.path.join(DATA, "mcp_safe_envelope_operator_*.json"))):
        idx = int(re.search(r"_(\d+)\.json$", path).group(1))
        with open(path) as f:
            out[idx] = json.load(f)["envelope"]
    return out


def cli_lines(project):
    path = os.path.join(DATA, f"cli_suggest_{project}.jsonl")
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def planned(rows):
    return [r for r in rows if r["subject"]["kind"] == "planned"]


def has_caveat(row, kind):
    return any(c["kind"] == kind for c in row["caveats"])


def side_margin(side, value, bound):
    if bound <= 0:
        return None
    return (bound - value) / bound if side == "ceiling" else (value - bound) / bound


class Field:
    """The force field of one op: bilinear on the grid the data holds."""

    def __init__(self, op):
        plane = op["plane"]
        self.aes = plane["field"]["aes"]
        self.aps = plane["field"]["aps"]
        self.force = plane["field"]["force_n"]
        self.budget = plane["budget_n"]
        self.cell = op["cell_mm"]
        self.top = op["top_mm"]
        self.role = plane["role"]

    def at(self, ae, ap):
        nc, nr = len(self.aes), len(self.aps)
        c = min(max(bisect.bisect_right(self.aes, ae) - 1, 0), nc - 2)
        r = min(max(bisect.bisect_right(self.aps, ap) - 1, 0), nr - 2)
        tx = (ae - self.aes[c]) / (self.aes[c + 1] - self.aes[c])
        ty = (ap - self.aps[r]) / (self.aps[r + 1] - self.aps[r])
        v = [self.force[r * nc + c], self.force[r * nc + c + 1],
             self.force[(r + 1) * nc + c], self.force[(r + 1) * nc + c + 1]]
        if any(x is None for x in v):
            return None
        return (v[0] * (1 - tx) + v[1] * tx) * (1 - ty) + (v[2] * (1 - tx) + v[3] * tx) * ty

    def depth_at_budget(self, ae):
        """The 100 % depth at the width ae, or None when the field does not
        reach the budget under its top (the body line is above the chart)."""
        hi = self.aps[-1]
        f = self.at(ae, hi)
        if f is None or f < self.budget:
            return None
        lo = self.aps[1] * 1e-3
        for _ in range(60):
            mid = 0.5 * (lo + hi)
            fm = self.at(ae, mid)
            if fm is None or fm < self.budget:
                lo = mid
            else:
                hi = mid
        return hi


def main():
    envs = mcp_envelopes()
    report = load("mcp_tool_load_report_operator.json")
    fields = load("field_operator.json")["ops"]
    cli = cli_lines("operator")
    if not envs or not fields or not cli:
        print("data/ is incomplete")
        return 2

    # Claim 1: one planned row per declared cut and planned limit; one
    # group row per gate and group.
    detail, ok, n1 = [], True, 0
    for idx, env in envs.items():
        rows = planned(env["limit_rows"])
        n1 += len(rows)
        pairs = [(r["subject"]["cut"], r["check"]["limit"]) for r in rows]
        if len(pairs) != len(set(pairs)):
            ok = False
            detail.append(f"idx {idx}: a (cut, limit) pair repeats")
        by_limit = {}
        for cut, limit in pairs:
            by_limit.setdefault(limit, set()).add(cut)
        cut_sets = {frozenset(v) for v in by_limit.values()}
        if len(cut_sets) != 1:
            ok = False
            detail.append(f"idx {idx}: the limits read different cuts: {by_limit}")
        detail.append(f"idx {idx}: {len(rows)} planned rows = {len(by_limit)} limits x "
                      f"{len(next(iter(cut_sets)))} cuts")
    n_tp = 0
    for tp in report["limit_rows"]:
        group_rows = [r for r in tp["rows"] if r["subject"]["kind"] == "group"
                      and r["subject"].get("material") is None]
        keys = [(r["check"]["limit"].replace("chip_min", "chip").replace("chip_max", "chip"),
                 r["subject"]["group"]) for r in group_rows if r["check"]["limit"] != "gantry_push"
                and not r["check"]["limit"].startswith("drill_")]
        if len(keys) != len(set(keys)):
            ok = False
            detail.append(f"{tp['toolpath_name']}: a (gate, group) pair repeats")
        gates = {g for g, _ in keys}
        for g in gates:
            groups = sorted(gr for gg, gr in keys if gg == g)
            if groups != sorted(GROUPS):
                ok = False
                detail.append(f"{tp['toolpath_name']}: gate {g} groups {groups}")
        n_tp += 1
        n1 += len(keys)
    detail.append(f"{n_tp} toolpaths of the load report: each gate has one row per cut group")
    claim("1", "One planned row per declared cut and planned limit; one group row per gate "
          "and cut group.", ok, detail, n1)

    # Claim 2: every margin is the side rule of its judged value and its
    # bound (section 6 claim 3). The judged value is the value, except on a
    # chip row that reads a stored whole-mm/min feed f: a floor judges the
    # high end of that feed's interval [f, f + k), value x (f + k) / f. The
    # script asserts the EXACT interval rule by the row's own facts
    # (P29-10z; the rule of `ConfiguredEntry::planned_chip_mm`):
    #   - f is the row's value x rpm x Z (the CLI record's stated rpm and
    #     flute count), and it is a whole feed;
    #   - one write, k = 1: a plunge or peck entry, a ramp or helix entry of
    #     an op with no entry feed (it runs at the plunge feed; f is the
    #     plunge feed), and a stated cut row (f is the op feed);
    #   - two writes, k = 1 + 1/L (P29-10 10i, lead ruling 1): a ramp or
    #     helix entry of an op that stores an entry feed (f is that feed),
    #     written from the stored cut feed over the body lift L at the
    #     full-feed width of the stated width. L is the op feed over
    #     rpm x Z x the stated chip of the full-feed cut (the planner cap
    #     where the op declares one, else the body).
    # The margin must equal the side rule of the judged value to 1e-9.
    detail, ok, n, n_entry = [], True, 0, 0
    entry_feeds = {}
    chip_limits = {"chip_min", "chip_max", "rubbing_floor"}
    records = {line["index"]: line for line in cli_lines("operator")}
    sources = [(idx, r) for idx, env in envs.items() for r in env["limit_rows"]]
    sources += [(None, r) for tp in report["limit_rows"] for r in tp["rows"]]

    def full_feed_lift(idx):
        rec = records.get(idx)
        rows = envs[idx]["limit_rows"] if idx in envs else []
        chips = {r["subject"].get("cut"): r["check"]["value"] for r in rows
                 if r["subject"]["kind"] == "stated_cut" and r["check"]["limit"] == "chip_max"
                 and r["check"]["value"] is not None}
        chip = chips.get("planner cap", chips.get("body"))
        if rec is None or chip is None:
            return None
        cur = rec["current"]
        return cur["feed_rate"] / (cur["spindle_rpm"] * rec["tool"]["flute_count"] * chip)

    for idx, r in sources:
        c = r["check"]
        if c["value"] is None or c["bound"] is None or c["margin"] is None:
            continue
        want = side_margin(c["side"], c["value"], c["bound"])
        if want is None:
            ok = False
            detail.append(f"{r['face']}: no side margin")
            continue
        n += 1
        if abs(want - c["margin"]) <= REL * max(1.0, abs(want)):
            continue
        kind = r["subject"]["kind"]
        cut = r["subject"].get("cut") or ""
        is_entry = kind in ("planned", "stated_cut") and "entry" in cut
        is_stated_cut = kind == "stated_cut" and not is_entry
        rec = records.get(idx)
        if (c["limit"] not in chip_limits or not (is_entry or is_stated_cut)
                or c["side"] != "floor" or rec is None):
            ok = False
            detail.append(f"idx {idx}: {r['face']}: margin {c['margin']} against {want}")
            continue
        cur = rec["current"]
        rpm, z = cur["spindle_rpm"], rec["tool"]["flute_count"]
        if is_stated_cut:
            # The stated cut reads the op feed over its own lift: f is the op
            # feed, one write.
            f, k, why = cur["feed_rate"], 1.0, "op feed, one write"
            lift = f / (rpm * z * c["value"])
            judged = c["value"] * (f + k) / f
        else:
            f = c["value"] * rpm * z
            if abs(f - round(f)) > 1e-6:
                ok = False
                detail.append(f"idx {idx}: {r['face']}: the entry feed {f} is not whole")
                continue
            f = round(f)
            two_writes = (cut in ("ramp entry", "helix entry")
                          and cur.get("entry_feed_rate") is not None)
            if two_writes:
                lift = full_feed_lift(idx)
                if lift is None or abs(f - cur["entry_feed_rate"]) > 1e-9:
                    ok = False
                    detail.append(f"idx {idx}: {r['face']}: f {f}, entry feed "
                                  f"{cur.get('entry_feed_rate')}, lift {lift}")
                    continue
                k, why = 1.0 + 1.0 / lift, f"entry feed, two writes, L {lift:.4f}"
            else:
                if abs(f - cur["plunge_rate"]) > 1e-9:
                    ok = False
                    detail.append(f"idx {idx}: {r['face']}: f {f} is not the plunge feed "
                                  f"{cur['plunge_rate']}")
                    continue
                k, why = 1.0, "plunge feed, one write"
            judged = c["value"] * (f + k) / f
        exact = side_margin(c["side"], judged, c["bound"])
        n_entry += 1
        entry_feeds.setdefault(idx, set()).add(f"{cut}: f {f:g}, k {k:.4f} ({why})")
        if abs(exact - c["margin"]) > REL * max(1.0, abs(exact)):
            ok = False
            detail.append(f"idx {idx}: {r['face']}: margin {c['margin']} against the interval "
                          f"rule {exact} ({why})")
    for idx, feeds in sorted(entry_feeds.items(), key=lambda kv: str(kv[0])):
        detail.append(f"idx {idx}: {sorted(feeds)}")
    detail.append(f"{n} rows with a value, a bound and a margin; {n - n_entry} on the plain side "
                  f"rule, {n_entry} chip floor rows (entries, stated cuts) on the exact interval "
                  f"rule of their stored feed")
    claim("2", "Every margin is the side rule of its judged value and its bound.", ok, detail[:20],
          n)

    # Claim 3: each planned mark sits at the plane point of its rows.
    detail, ok, n = [], True, 0
    for idx, env in envs.items():
        marks = {m["cut"]: m["at"] for m in env["plane_marks"]["proposal"]["marks"]}
        for r in planned(env["limit_rows"]):
            at = r["subject"]["at"]
            cut = r["subject"]["cut"]
            if (at is None) != (cut not in marks) or (at is not None and at != marks[cut]):
                ok = False
                detail.append(f"idx {idx}: {cut}: row {at} mark {marks.get(cut)}")
            n += at is not None
    detail.append(f"{n} planned rows with a plane point, each at the mark of its cut")
    claim("3", "Each planned mark sits at the (ae, ap) of its rows.", ok, detail, n)

    # Claim 4 (ruling R3): a mark takes its tone from its row. For the body
    # cut the force-budget row exceeds exactly when the mark is above the
    # body 100 % contour, within one cell. Two kinds of body mark: the
    # planned body mark at the proposal (its planned force-budget row) and
    # the STATED body mark at the stated point (its stated force-budget
    # row; review item 1: the stated mark of idx 1 is 387 % over the force
    # cap, a row that exceeds). The claim needs a row that exceeds and a row
    # that does not, each decided by the side of the line (not by the
    # one-cell tolerance); with no such row it is NOT TESTED.
    detail, ok, n, tol = [], True, 0, 0
    side_exceeds = side_within = 0
    for idx, env in envs.items():
        op = fields[idx]
        if not op.get("plane"):
            continue
        field = Field(op)
        cases = []
        for r in planned(env["limit_rows"]):
            if r["subject"]["cut"] != "body" or r["check"]["limit"] != "force_budget":
                continue
            if r["subject"]["at"] is None or r["check"]["bound"] is None:
                continue
            cases.append(("planned", r["subject"]["at"], r))
        stated_marks = {m["cut"]: m["at"] for m in env["plane_marks"]["stated"]["marks"]}
        for r in env["limit_rows"]:
            # P29-10 10i: the stated body force-budget row is the stated cut
            # row of the body (`stated_cut`).
            if (r["subject"]["kind"] != "stated_cut" or r["subject"].get("cut") != "body"
                    or r["check"]["limit"] != "force_budget"
                    or r["check"]["bound"] is None or "body" not in stated_marks):
                continue
            cases.append(("stated", stated_marks["body"], r))
        for kind, at, r in cases:
            d = field.depth_at_budget(at["ae_mm"])
            exceeds = r["check"]["state"]["state"] == "exceeds"
            if d is None:
                above = False
                near = at["ap_mm"] > field.top - field.cell
            else:
                above = at["ap_mm"] > d
                near = abs(at["ap_mm"] - d) <= field.cell
            n += 1
            if near:
                tol += 1
            elif above != exceeds:
                ok = False
            else:
                side_exceeds += exceeds
                side_within += not exceeds
            detail.append(
                f"idx {idx}: {kind} body mark ap {at['ap_mm']:.3f} at ae {at['ae_mm']:.3f}; "
                f"contour {'above the chart' if d is None else f'{d:.3f}'}; "
                f"row {r['face']!r} {r['check']['state']['state']}; cell {field.cell:.3f}"
                + ("; within one cell" if near else ""))
    # The page example: the helix of idx 1 sits above the body contour at
    # D and its row (its own chip) is inside its own bound.
    ex = None
    if 1 in envs and fields[1].get("plane"):
        field = Field(fields[1])
        for r in planned(envs[1]["limit_rows"]):
            if r["subject"]["cut"] == "helix entry" and r["check"]["limit"] == "force_budget":
                at = r["subject"]["at"]
                d = field.depth_at_budget(at["ae_mm"])
                ex = (at["ap_mm"], d, r["check"]["bound"], r["check"]["state"]["state"])
    if ex is None or ex[1] is None or not (ex[0] > ex[1] and ex[3] == "within"):
        ok = False
        detail.append(f"the idx 1 helix example does not hold: {ex}")
    else:
        detail.append(f"idx 1 helix: mark ap {ex[0]:.3f} above the body contour {ex[1]:.3f} at D; "
                      f"row bound {ex[2]:.3f} (its own chip), state {ex[3]}: the row is the truth")
    detail.append(f"{n} body marks: {tol} decided by the one-cell tolerance; by the side of the "
                  f"line {side_exceeds} rows that exceed (mark above) and {side_within} rows "
                  f"within (mark under)")
    claim("4", "A mark takes its tone from its row; a body mark above the 100 % contour "
          "(beyond one cell) is a row that exceeds.", ok, detail,
          n if side_exceeds and side_within else 0)

    # Claims 5a and 5b (ruling R2).
    d5a, ok5a, d5b, ok5b = [], True, [], True
    caps = n5b = n5a = 0
    for idx, env in envs.items():
        op = fields[idx]
        cols = env["force_budget_columns"]
        if not cols or not op.get("plane"):
            continue
        field = Field(op)
        n_cols = n_above = n_caps = 0
        for col in cols:
            if col["sets"] != "force_budget":
                caps += 1
                n_caps += 1
                continue
            ae, ap = col["ae_mm"], col["ap_mm"]
            if ae > field.aes[-1] + 1e-9 or ap is None:
                continue
            d = field.depth_at_budget(ae)
            if d is None and ap > field.top + field.cell:
                n_above += 1
                continue  # the edge and the body line are both above the chart
            n_cols += 1
            if d is None:
                body_ok = True  # the body line is above the chart top, the edge is under it
            else:
                body_ok = ap <= d + field.cell
            if not body_ok:
                ok5a = False
                d5a.append(f"idx {idx}: ae {ae:.3f}: edge {ap:.3f} above the contour {d:.3f}")
            if col["cut"] == "body" and d is not None:
                n5b += 1
                if abs(ap - d) > field.cell:
                    ok5b = False
                    d5b.append(f"idx {idx}: ae {ae:.3f}: edge {ap:.3f}, contour {d:.3f}")
        n5a += n_cols
        d5a.append(f"idx {idx}: {n_cols} columns judged, {n_above} above the chart top (cell "
                   f"{field.cell:.3f} mm, top {field.top:.3f} mm)"
                   + ("" if n_cols else
                      " NOT TESTED on this op: every column is on a geometric cap"
                      if n_caps == len(cols) else
                      " NOT TESTED on this op: no force-budget column is under the chart top"))
    d5a.append(f"{caps} columns on a geometric cap (flute top or depth floor) are outside 5b")
    claim("5a", "In every column the force-budget edge is never above the body 100 % contour "
          "by more than one cell.", ok5a, d5a, n5a)
    d5b.append(f"{n5b} columns where the body cut sets the edge")
    claim("5b", "Where the body cut sets the edge, the edge is the body 100 % contour within "
          "one cell.", ok5b, d5b, n5b)

    # Claim 6: WorstCase and LowerBound hold on their rows only. Review
    # 2026-11-15 F2 (lead ruling): the worst case holds only on a ramp row
    # whose value reads the ramp's own depth: the three depth ceilings and
    # the power. A chip, rubbing-floor or vertical-rate row is exact.
    depth_limits = {"force_budget", "tool_deflection", "vendor_depth_table", "spindle_power"}
    detail, ok, nw, nl, nexact = [], True, 0, 0, 0
    op_types = {op["index"]: op["op_type"] for op in fields}
    for line in cli:
        env = line["profile"].get("envelope") or {}
        for r in planned(env.get("limit_rows", [])):
            at = r["subject"]["at"]
            ramp_at_own = r["subject"]["cut"] == "ramp entry" and at is not None and at["own_depth"]
            ramp_own = ramp_at_own and r["check"]["limit"] in depth_limits
            nexact += ramp_at_own and not ramp_own
            if has_caveat(r, "worst_case") != ramp_own:
                ok = False
                detail.append(f"idx {line['index']}: {r['face']}: worst_case {has_caveat(r, 'worst_case')}")
            nw += ramp_own
            if has_caveat(r, "lower_bound"):
                nl += 1
                if op_types.get(line["index"]) != "ProjectCurve" or (at and at["own_depth"]):
                    ok = False
                    detail.append(f"idx {line['index']}: lower_bound on {r['face']}")
    detail.append(f"{nw} ramp depth or power rows at an own depth carry worst_case; {nexact} ramp "
                  f"chip, rubbing or vertical-rate rows carry none; {nl} lower_bound rows, all "
                  "on the body plane of a Project Curve")
    claim("6", "WorstCase holds exactly on a ramp entry row at its own depth whose value reads "
          "that depth (a depth ceiling or the power); LowerBound only on the body plane of a "
          "Project Curve.", ok, detail, nw + nl if nw and nl and nexact else 0)

    # Claim 7: the CLI rows equal the MCP rows for the same op.
    detail, ok = [], True
    by_index = {line["index"]: line for line in cli}
    for idx, env in envs.items():
        c = by_index[idx]["profile"]["envelope"]
        for key in ["limit_rows", "plane_marks", "force_budget_columns", "outside_cut"]:
            if c[key] != env[key]:
                ok = False
                detail.append(f"idx {idx}: {key} differs")
        detail.append(f"idx {idx}: {len(env['limit_rows'])} rows, marks, columns and the "
                      "outside cut equal")
    claim("7", "The CLI rows equal the MCP rows for the same op.", ok, detail, len(envs))

    # Claim 9: a drill gate row states its population in holes.
    detail, ok, n = [], True, 0
    for tp in report["limit_rows"]:
        for r in tp["rows"]:
            pop = r["check"]["population"]
            if not r["check"]["limit"].startswith("drill_") or pop is None:
                continue
            n += 1
            words = f"{pop['contributing']} of {pop['offered']} {pop['unit']}"
            if pop["unit"] != "holes" or words not in r["detail"]:
                ok = False
                detail.append(f"{tp['toolpath_name']}: {r['face']}: {pop} / {r['detail'][:80]}")
    detail.append(f"{n} drill gate rows with a population, each 'N of M holes' in its detail")
    claim("9", "A drill gate row states the unit of its population: holes.", ok, detail, n)

    # Claim 10: own depth in the detail exactly when the point holds one.
    detail, ok, n = [], True, 0
    for idx, env in envs.items():
        for r in planned(env["limit_rows"]):
            at = r["subject"]["at"]
            own = at is not None and at["own_depth"]
            says = "own depth" in r["detail"]
            n += own
            if own != says:
                ok = False
                detail.append(f"idx {idx}: {r['face']}: point own depth {own}, detail {says}")
    detail.append(f"{n} planned rows at an own depth; each detail names it, and no other does")
    claim("10", "A planned row names an own depth exactly when its plane point holds one.",
          ok, detail, n)

    # Claim 8: every claim the page writes has its assert number beside it.
    detail, ok = [], True
    with open(os.path.join(HERE, "index.html")) as f:
        page = f.read()
    tags = re.findall(r'class="claim"[^>]*data-claim="([^"]+)"', page)
    ids = {cid for cid, *_ in results}
    missing = sorted(ids - set(tags))
    unknown = sorted(set(tags) - ids - {"8"})
    if missing or unknown or not tags:
        ok = False
    detail.append(f"{len(tags)} claim paragraphs on the page; asserted ids {sorted(ids | {'8'})}; "
                  f"missing on the page {missing}; on the page with no assert {unknown}")
    claim("8", "Every claim the page writes has an assert number beside it.", ok, detail,
          len(tags))

    bad = [cid for cid, _, good, _ in results if not good]
    print(f"\n{len(results) - len(bad)} of {len(results)} claims PASS" +
          (f"; FAIL: {', '.join(bad)}" if bad else ""))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
