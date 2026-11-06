"""Re-derive each claim of the old review (../assert_claims.py CLAIMS) at the current HEAD.

The old checker reads live code lines, the FM1 golden and the LUT JSON; P16 moved some of them.
This wrapper evaluates each claim and compares it with the frozen value in ../claims.json.
Read-only. Prints one line per claim that changed or no longer derives.
"""
import json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
OLD = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, OLD)
os.chdir(OLD)
import assert_claims as ac  # noqa: E402
frozen = json.load(open(os.path.join(OLD, "claims.json")))
same = changed = 0
for cid, fn, fmt, files in ac.CLAIMS:
    try:
        v = fn()
        shown = fmt.format(int(round(v)) if fmt == "{:d}" and isinstance(v, float) else v)
    except Exception as e:  # noqa: BLE001
        shown = f"NO LONGER DERIVES ({type(e).__name__})"
    if shown == frozen[cid]["shown"]:
        same += 1
    else:
        changed += 1
        print(f"{cid}: {frozen[cid]['shown']} -> {shown}")
print(f"same {same}, changed {changed}")
