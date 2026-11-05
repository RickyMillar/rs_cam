"""P11b L9: write index.html from results.json (self-contained, no network).

Run: python3 -I build_page.py   (after python3 -I run_suggest.py)

Every number on the page comes from results.json (the CLI run) or from ../results.json
(step 0). The claim sentences carry ids; assert_claims.py checks each id.
"""
import html
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
R = json.load(open(os.path.join(HERE, "results.json")))


def pct(f):
    return "n/a" if f is None else f"{100 * f:.1f} %"


def rt(x):
    return "n/a" if x is None else f"{x:.2f}"


def fam_rows():
    out = []
    for key, v in R["per_family"].items():
        g, vendor = key.split(":")
        t, r, u = v["target"], v["reference"], v["target_unbound"]
        out.append(
            f"<tr><td>{g}</td><td>{html.escape(vendor)}</td><td>{t['passed']} / {t['n']}</td>"
            f"<td>{pct(t['frac'])}</td><td>{u['passed']} / {u['n']}</td><td>{pct(u['frac'])}</td>"
            f"<td>{r['passed']} / {r['n']}</td><td>{pct(r['frac'])}</td>"
            f"<td>{rt(v['median_ratio_target'])}</td><td>{rt(v['median_ratio_reference'])}</td>"
            f"<td>{v['feed_capped']}</td><td>{v['rpm_lowered']}</td><td>{v['power_limited']}</td>"
            f"<td>{v['errors']}</td></tr>")
    return "\n".join(out)


def case_rows(group):
    out = []
    for c in R["cases"]:
        if c["group"] != group:
            continue
        s = c["suggest"]
        lo, hi = c["tgt_band"]
        band = f"{hi:.4f}" if lo is None else f"{lo:.4f}–{hi:.4f}"
        chip = "—" if s.get("chip") is None else f"{s['chip']:.4f}"
        feed = "—" if s.get("feed") is None else f"{s['feed']:.0f}"
        rpm = "—" if s.get("rpm") is None else f"{s['rpm']:.0f}"
        cls = {True: "p", False: "f", None: "n"}[c["passes"]]
        mark = {True: "in", False: "out", None: "n/a"}[c["passes"]]
        flags = " ".join(x for x, on in (("cap", s.get("feed_capped")), ("rpm↓", s.get("rpm_lowered")),
                                         ("power", s.get("power_limited"))) if on)
        out.append(
            f"<tr><td>{html.escape(c['label'])}</td><td>{html.escape(c['material'])}</td>"
            f"<td>{band}</td><td>{chip}</td><td class='{cls}'>{mark}</td><td>{feed}</td><td>{rpm}</td>"
            f"<td>{html.escape(str(s.get('row', s.get('error', ''))))}</td><td>{flags}</td></tr>")
    return "\n".join(out)


g = R["groups"]
W, D = g["W"], g["D"]
page = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>P11b Suggest Proof</title>
<style>
:root{{--bg:#fbfaf7;--fg:#1d1f22;--mut:#5f6670;--line:#d9d6cf;--card:#ffffff;--pass:#2f7d4f;--fail:#b3412f;--na:#9aa0a6;--acc:#2b5d9b}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{--bg:#16181b;--fg:#e7e5e0;--mut:#a3a9b1;--line:#34383e;--card:#1e2125;--pass:#5dbb83;--fail:#e2735f;--na:#6d737a;--acc:#7fa8e0}}}}
:root[data-theme="dark"]{{--bg:#16181b;--fg:#e7e5e0;--mut:#a3a9b1;--line:#34383e;--card:#1e2125;--pass:#5dbb83;--fail:#e2735f;--na:#6d737a;--acc:#7fa8e0}}
body{{background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif;margin:0}}
main{{max-width:1040px;margin:0 auto;padding:24px 16px 64px}}
h1{{font-size:1.5rem;margin:0 0 4px}}h2{{font-size:1.15rem;margin:32px 0 8px;border-bottom:1px solid var(--line);padding-bottom:4px}}
.mut{{color:var(--mut)}}.card{{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:12px 14px;margin:10px 0}}
.badge{{display:inline-block;padding:2px 10px;border-radius:12px;font-weight:600;color:#fff}}.fail{{background:var(--fail)}}.pass{{background:var(--pass)}}
table{{border-collapse:collapse;width:100%;font-size:13px}}th,td{{border-bottom:1px solid var(--line);padding:4px 6px;text-align:right}}th:first-child,td:first-child,td:nth-child(2){{text-align:left}}
td.p{{color:var(--pass);font-weight:600}}td.f{{color:var(--fail)}}td.n{{color:var(--na)}}
.scroll{{overflow-x:auto;max-height:520px;overflow-y:auto}}code{{font-size:12.5px}}
.claim{{border-left:3px solid var(--acc);padding-left:8px;margin:6px 0}}
.claim::before{{content:attr(data-claim);font:600 11px ui-monospace,monospace;color:var(--acc);margin-right:6px}}
</style>
</head>
<body>
<main>
<h1>P11b: where the shipped Suggest chip sits against the printed bands</h1>
<p class="mut">Feeds contract P11b, decision L9. Script <code>run_suggest.py</code> runs the release CLI <code>suggest --json</code> over the step 0 case set (<code>../results.json</code>); <code>build_page.py</code> writes this page from <code>results.json</code>; <code>assert_claims.py</code> checks each claim id. The pass rule is the step 0 rule (<code>../PREREGISTRATION.md</code>), applied as written, with no tuning after the run.</p>

<div class="card">
<p><b>Question.</b> Does the shipped Suggest (chip first, chip thinning, the depth table as a limit, not a feed factor) put its chip inside the printed band of each step 0 target case?</p>
<p><b>Verdict against the step 0 rule:</b> <span class="badge {'pass' if R['verdict'] == 'PASS' else 'fail'}">{R['verdict']}</span></p>
<p class="claim" data-claim="A01">The step 0 case set has {R['n_cases']} cases: group W {W['n']} testable, group D {D['n']} testable.</p>
<p class="claim" data-claim="A02">Group W (printed width): the Suggest chip is inside the target band in {W['passed']} of {W['n']} cases ({pct(W['frac'])}).</p>
<p class="claim" data-claim="A03">Group D (printed depth table, 2 x D and 3 x D): the Suggest chip is inside the target band in {D['passed']} of {D['n']} cases ({pct(D['frac'])}).</p>
<p class="claim" data-claim="A04">The verdict is {R['verdict']}: the rule needs {pct(R['pass_fraction'])} in each group.</p>
</div>

<h2>1. How to read the result</h2>
<div class="card">
<p>The step 0 target band of a group D case is the row band x 0.75 at 2 x D and x 0.50 at 3 x D. P11b decision L6 removes that scale from the chip on purpose: Suggest holds the chip in the printed band at every depth, and the depth table is a drawn envelope limit ("vendor depth ceiling (width not printed)") with a warning past it. Thus a group D case fails by design when the Suggest chip is the printed chip. The "reference" columns show the same rows at their own 1 x D slot condition, where the printed band applies with no scale.</p>
<p class="claim" data-claim="A05">At the 1 x D reference condition of the group D rows, the Suggest chip is inside the row's printed band in {sum(v['reference']['passed'] for k, v in R['per_family'].items() if k.startswith('D:'))} of {sum(v['reference']['n'] for k, v in R['per_family'].items() if k.startswith('D:'))} cases.</p>
<p class="claim" data-claim="A06">{sum(1 for c in R['cases'] if c['suggest'].get('error') == 'no record')} cases are not testable: their target depth is above the Pocket depth range (the CLI refuses a depth per pass above 50 mm).</p>
<p class="claim" data-claim="A07">{sum(1 for c in R['cases'] if str(c['suggest'].get('error', '')).startswith('Suggest refused'))} cases are not testable: Suggest refuses the cell (no published figure for the tool size in the material).</p>
<p>Suggest picks its own vendor row for each tool and material (the generic species of the family). The comparison is always against the step 0 case's printed band, so a different row, a hardness scale or a size claim can put the chip off the case's band.</p>
</div>

<h2>2. Per vendor family</h2>
<div class="scroll"><table>
<tr><th>group</th><th>vendor</th><th>target in band</th><th>%</th><th>target in band, no limit bound</th><th>%</th><th>reference in band</th><th>%</th><th>median chip / band mid (target)</th><th>median chip / band mid (reference)</th><th>feed cap</th><th>rpm lowered</th><th>power limited</th><th>errors</th></tr>
{fam_rows()}
</table></div>
<p class="claim" data-claim="A08">The project default machine gives a rated power of {', '.join(f'{x:g}' for x in R['machine']['available_kw'])} kW at the shipped RPM and a cutting-feed ceiling of {', '.join(f'{x:g}' for x in R['machine']['feed_ceiling_mm_min'])} mm/min (both read from the CLI output). "No limit bound" keeps the cases where none of the feed cap, the RPM descent at the cap or the spindle power moved the Suggest feed: the same cases and the same rule, so a reader can separate a machine effect from the chip model.</p>
<p class="mut">"feed cap", "rpm lowered" and "power limited" count the target cases where the machine cutting-feed ceiling, the Q1 arm L or the spindle power bound the Suggest feed (decision L3: no cap constant on the chip-thinning lift; these limits bound it).</p>

<h2>3. Group W cases</h2>
<div class="scroll"><table>
<tr><th>case</th><th>material</th><th>printed band</th><th>Suggest chip</th><th>in band</th><th>feed</th><th>rpm</th><th>Suggest row</th><th>limits</th></tr>
{case_rows('W')}
</table></div>

<h2>4. Group D cases</h2>
<div class="scroll"><table>
<tr><th>case</th><th>material</th><th>target band</th><th>Suggest chip</th><th>in band</th><th>feed</th><th>rpm</th><th>Suggest row</th><th>limits</th></tr>
{case_rows('D')}
</table></div>
</main>
</body>
</html>
"""
open(os.path.join(HERE, "index.html"), "w").write(page)
print("index.html written,", len(page), "bytes")
