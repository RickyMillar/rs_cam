"""Mechanical debt instruments. Output: evidence/*.md under planning/tech_debt_2026-09-16."""
import re, os, sys, collections, pathlib
ROOT = pathlib.Path('/home/ricky/personal_repos/rs_cam'); EV = ROOT/'planning/tech_debt_2026-09-16/evidence'
CRATES = ['rs_cam_core','rs_cam_viz','rs_cam_cli','rs_cam_mcp']
def rs_files(sub):
    for c in CRATES:
        for dp,_,fs in os.walk(ROOT/'crates'/c/sub):
            for f in fs:
                if f.endswith('.rs'): yield pathlib.Path(dp)/f
SRC = list(rs_files('src')); ALL = SRC + list(rs_files('tests')) + list(rs_files('benches')) + list(rs_files('examples'))
text = {p: p.read_text(errors='replace') for p in ALL}
# --- test-scope helper (inline cfg(test) mod ... { } to EOF approx)
def prod_text(s):
    m = re.search(r'#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*mod\s+\w+\s*\{', s)
    return s[:m.start()] if m else s
# ---------- 1. dead public surface
pat = re.compile(r'^\s*pub(?:\((?:crate|super|in [\w:]+)\))?\s+(?:const\s+)?(?:unsafe\s+)?(?:async\s+)?(fn|struct|enum|const|static|type|trait)\s+([A-Za-z_][A-Za-z0-9_]*)', re.M)
defs = []  # (name, kind, file, line, visibility)
for p in SRC:
    s = prod_text(text[p])
    for m in pat.finditer(s):
        vis = 'pub' if m.group(0).lstrip().startswith('pub ') else 'pub(x)'
        line = s.count('\n', 0, m.start()) + 1
        defs.append((m.group(2), m.group(1), p, line, vis))
big = '\n'.join(text.values())
counts = collections.Counter()
for name,_,_,_,_ in defs:
    pass
# count word occurrences across everything once
word_re = re.compile(r'\b[A-Za-z_][A-Za-z0-9_]*\b')
for w in word_re.findall(big): counts[w] += 1
dead = [(n,k,p,l,v) for n,k,p,l,v in defs if counts[n] <= 1 and n not in ('main','new','default')]
# also 'only referenced within its own file' (2..3 occurrences, all in same file)
own = []
for n,k,p,l,v in defs:
    if 2 <= counts[n] <= 3:
        c_here = len(re.findall(r'\b'+re.escape(n)+r'\b', text[p]))
        if c_here == counts[n] and v == 'pub': own.append((n,k,p,l,v))
with open(EV/'dead_pub_surface.md','w') as f:
    f.write(f"# Dead public surface (mechanical)\n\nDefinitions: {len(defs)} pub items in production code. Word-count instrument over crates/*/{{src,tests,benches,examples}}; a name that occurs once in the whole workspace is its own definition and nothing else. Trait-impl methods and macro-generated uses are blind spots: VERIFY each row with rg before acting.\n\n")
    f.write(f"## Zero references anywhere ({len(dead)})\n\n| kind | name | file:line | vis |\n|---|---|---|---|\n")
    for n,k,p,l,v in sorted(dead, key=lambda r:(str(r[2]),r[3])): f.write(f"| {k} | `{n}` | `{p.relative_to(ROOT)}:{l}` | {v} |\n")
    f.write(f"\n## `pub` but referenced only inside its own file ({len(own)}) — visibility too wide, or dead behind a same-file caller\n\n| kind | name | file:line |\n|---|---|---|\n")
    for n,k,p,l,v in sorted(own, key=lambda r:(str(r[2]),r[3])): f.write(f"| {k} | `{n}` | `{p.relative_to(ROOT)}:{l}` |\n")
# ---------- 2. legacy residue
lr = re.compile(r'legacy|Legacy|LEGACY|migrat|compat|deprecated|Deprecated|pre-v[0-9]|fallback loader|_legacy', re.I)
rows = []
for p in SRC:
    for i,line in enumerate(text[p].split('\n'),1):
        if lr.search(line): rows.append((p,i,line.strip()[:140]))
byfile = collections.Counter(str(p.relative_to(ROOT)) for p,_,_ in rows)
with open(EV/'legacy_residue.md','w') as f:
    f.write(f"# Legacy / compat / migration residue in production sources ({len(rows)} lines, {len(byfile)} files)\n\nRuling: no legacy project-format support; judge every hit. Historical notes that say something WAS removed are fine; code that still reads or writes an old shape is not.\n\n## Per file\n\n| hits | file |\n|---|---|\n")
    for fp,c in byfile.most_common(): f.write(f"| {c} | `{fp}` |\n")
    f.write("\n## Lines\n\n")
    for p,i,l in rows: f.write(f"- `{p.relative_to(ROOT)}:{i}` {l}\n")
# ---------- 3. inventory
allows, todos, sizes = [], [], []
fn_re = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?fn\s+([A-Za-z_]\w*)', re.M)
longfns = []
for p in SRC:
    s = text[p]; ps = prod_text(s)
    sizes.append((len(s.split('\n')), p))
    for i,line in enumerate(ps.split('\n'),1):
        if re.search(r'#!?\[allow\(', line): allows.append((p,i,line.strip()[:120]))
        if re.search(r'\b(TODO|FIXME|XXX|HACK)\b', line): todos.append((p,i,line.strip()[:120]))
    # crude fn length: from fn line to next fn line at same-or-lower indent
    ms = list(fn_re.finditer(ps)); lines = ps.split('\n')
    for a,b in zip(ms, ms[1:]+[None]):
        start = ps.count('\n',0,a.start())+1
        end = (ps.count('\n',0,b.start())+1) if b else len(lines)
        if end-start >= 250: longfns.append((end-start, a.group(1), p, start))
with open(EV/'inventory.md','w') as f:
    f.write("# Inventory (mechanical)\n\n## Largest production files (top 25)\n\n| lines | file |\n|---|---|\n")
    for n,p in sorted(sizes, reverse=True)[:25]: f.write(f"| {n} | `{p.relative_to(ROOT)}` |\n")
    f.write(f"\n## Functions >= 250 lines (crude: to the next `fn`) ({len(longfns)})\n\n| lines | fn | file:line |\n|---|---|---|\n")
    for n,name,p,l in sorted(longfns, reverse=True)[:60]: f.write(f"| {n} | `{name}` | `{p.relative_to(ROOT)}:{l}` |\n")
    f.write(f"\n## `allow(` in production code ({len(allows)})\n\n")
    for p,i,l in allows: f.write(f"- `{p.relative_to(ROOT)}:{i}` {l}\n")
    f.write(f"\n## TODO / FIXME / XXX / HACK in production code ({len(todos)})\n\n")
    for p,i,l in todos: f.write(f"- `{p.relative_to(ROOT)}:{i}` {l}\n")
print("defs",len(defs),"dead",len(dead),"own-file-only",len(own),"legacy lines",len(rows),"allows",len(allows),"todos",len(todos),"longfns",len(longfns))

# ---------- 4. test-only public API: production refs == 1 (its own def), test refs > 0
prod_big = '\n'.join(text[p] for p in SRC); test_big = '\n'.join(text[p] for p in ALL if p not in set(SRC))
pc, tc = collections.Counter(word_re.findall(prod_big)), collections.Counter(word_re.findall(test_big))
testonly = [(n,k,p,l,v) for n,k,p,l,v in defs if pc[n] <= 1 and tc[n] > 0 and v == 'pub']
with open(EV/'test_only_pub_api.md','w') as f:
    f.write(f"# `pub` items referenced only from tests/benches/examples ({len(testonly)})\n\nProduction word count is 1 (the definition) and the test-side count is > 0. Some are deliberate fixtures (`test_fixture`); the rest are test-only API leaking as public surface. Verify with rg; same blind spots as dead_pub_surface.md.\n\n| kind | name | file:line | test refs |\n|---|---|---|---|\n")
    for n,k,p,l,v in sorted(testonly, key=lambda r:(str(r[2]),r[3])): f.write(f"| {k} | `{n}` | `{p.relative_to(ROOT)}:{l}` | {tc[n]} |\n")
# ---------- 5. allows without a SAFETY comment within 3 lines above (production code)
nosafety = []
for p in SRC:
    lines = prod_text(text[p]).split('\n')
    for i,line in enumerate(lines):
        if re.search(r'#!?\[allow\(', line):
            window = '\n'.join(lines[max(0,i-3):i+1])
            if 'SAFETY' not in window and 'safety' not in window.lower():
                nosafety.append((p,i+1,line.strip()[:110]))
byl = collections.Counter(re.search(r'allow\(([^)]*)\)', l).group(1) if re.search(r'allow\(([^)]*)\)', l) else '?' for _,_,l in nosafety)
with open(EV/'allows_without_safety.md','w') as f:
    f.write(f"# `allow(` in production code with no SAFETY comment within 3 lines ({len(nosafety)} of {len(allows)})\n\nCLAUDE.md: prefer a local, documented allow with a `SAFETY:` comment. An undocumented allow is debt by the repo's own definition. `allow(dead_code)` rows are dead production code regardless of comment.\n\n## By lint\n\n| count | lint |\n|---|---|\n")
    for l,c in byl.most_common(): f.write(f"| {c} | `{l}` |\n")
    f.write("\n## Rows\n\n")
    for p,i,l in nosafety: f.write(f"- `{p.relative_to(ROOT)}:{i}` {l}\n")
print("testonly",len(testonly),"allows w/o SAFETY",len(nosafety))
