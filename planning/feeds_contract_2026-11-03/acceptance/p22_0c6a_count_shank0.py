"""Count every file that states a shank of 0 (P22-0c-6a, ruling 6)."""
import json, os, re, sys, tomllib
ROOTS = ["crates", "planning/fixtures", "planning/rivmap_block_job", "fixtures", "test_data"]
FORMS = ("stated", "catalogue", "estimated")
def zero(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool) and v == 0
def walk(node, path, hits):
    if isinstance(node, dict):
        for k, v in node.items():
            p = f"{path}.{k}" if path else k
            if k == "shank_diameter":
                if zero(v):
                    hits.append((p, "bare 0"))
                elif isinstance(v, dict):
                    for f in FORMS:
                        if zero(v.get(f)):
                            hits.append((p, f"{f} = 0"))
            walk(v, p, hits)
    elif isinstance(node, list):
        for i, v in enumerate(node):
            walk(v, f"{path}[{i}]", hits)
RS = [re.compile(p) for p in [
    r"set_shank_diameter\(\s*ToolFact::\w+(\s*\{\s*value:)?\s*\(?\s*0(\.0+)?(_f64)?\b",
    r"shank_diameter\s*:\s*ToolFact::\w+(\s*\{\s*value:)?\s*\(?\s*0(\.0+)?(_f64)?\b",
    r"shank_diameter\s*=\s*\{?\s*(stated|catalogue|estimated)\s*=\s*0(\.0+)?\b",
    r"\"shank_diameter\"\s*:\s*\{?\s*\"?(stated|catalogue|estimated)?\"?\s*:?\s*0(\.0+)?\b",
    r"ShankDiameter[^\n]{0,40}\b0\.0\b",
]]
total = {}
for root in ROOTS:
    for dp, dn, fn in os.walk(root):
        if "/target" in dp: continue
        for f in fn:
            p = os.path.join(dp, f)
            ext = f.rsplit(".", 1)[-1]
            hits = []
            try:
                if ext == "toml":
                    with open(p, "rb") as h: walk(tomllib.load(h), "", hits)
                elif ext == "json":
                    with open(p) as h: walk(json.load(h), "", hits)
            except Exception as e:
                if ext in ("toml", "json") and "shank_diameter" in open(p, errors="ignore").read():
                    print(f"PARSE-FAIL {p}: {e}", file=sys.stderr)
            if ext in ("rs", "toml", "json", "py"):
                txt = open(p, errors="ignore").read()
                for i, line in enumerate(txt.splitlines(), 1):
                    for rx in RS:
                        if rx.search(line):
                            hits.append((f"line {i}", line.strip()[:110]))
            if hits:
                total[p] = sorted(set(hits))
for p, hs in sorted(total.items()):
    print(p)
    for h in hs: print("   ", h)
print(f"FILES: {len(total)}")
