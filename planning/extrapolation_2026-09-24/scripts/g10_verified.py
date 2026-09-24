#!/usr/bin/env python3
"""G10 (entry parameters): merge the fetch parts and the verifier verdicts.

Reads fetch/G10/parts/{wood,metal,hobby}_{sources,statements}.json and
fetch/G10/verify/{a,b}_verdicts.json. Writes:
  - fetch/G10/sources.json     every source record, with the verifier's
                               rehash result added as `verify`;
  - fetch/G10/statements.json  every statement with its `verdict`.
The corrections below are typed here from the verdicts; the script
asserts that each one names a statement that the verifier did not confirm.
A `wrong` statement carries the corrected field and the original value in
`corrected_from`. A `grade_wrong` statement carries the corrected mapping.
No statement is dropped (G10 has no not_found verdict; the script fails if
one appears).

Usage: python3 g10_verified.py   (then g10_check_statements.py --merged)
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
G10 = os.path.normpath(os.path.join(HERE, "..", "fetch", "G10"))
PARTS = ["wood", "metal", "hobby"]
VERDICTS = ["a", "b"]

PLASTICS_LABEL = {
    "printed": "Plastics (Non-Filled, Glass Filled, Carbon Fiber, G10); Non-Ferrous "
    "(Aluminum, Magnesium, Copper Alloys); cast iron; steels; high temp alloys",
    "normalised": "plastic_and_metal",
}

# statement_id -> function that corrects the statement in place
CORRECTIONS = {
    "g10-metal-harvey-sf18700-plunge": lambda s: _set(
        s, "diameter_mm", [0.381, 12.7],
        derived_fix=("diameter_mm", [0.381, 12.7], "0.015 in x 25.4 .. 0.500 in x 25.4"),
    ),
    "g10-metal-harvey-sf25000-plunge": lambda s: _set(s, "material", PLASTICS_LABEL),
    "g10-metal-harvey-sf25000-ramp-pref": lambda s: _set(s, "material", PLASTICS_LABEL),
    "g10-wood-vortex-veining-plunge": lambda s: _set(s, "tool_family", "other"),
    "g10-hobby-019": lambda s: _set(s, "parameter", "entry_general"),
}


def _set(s, key, value, derived_fix=None):
    s.setdefault("corrected_from", {})[key] = s.get(key)
    s[key] = value
    if derived_fix:
        q, v, f = derived_fix
        for d in s.get("derived", []):
            if d.get("quantity") == q:
                d["value"], d["formula"] = v, f


def main():
    sources, statements = [], []
    for p in PARTS:
        for s in json.load(open(os.path.join(G10, "parts", f"{p}_sources.json"))):
            s["part"] = p
            sources.append(s)
        for st in json.load(open(os.path.join(G10, "parts", f"{p}_statements.json"))):
            st["part"] = p
            statements.append(st)
    src_verify, st_verdict = {}, {}
    for v in VERDICTS:
        doc = json.load(open(os.path.join(G10, "verify", f"{v}_verdicts.json")))
        for s in doc["sources"]:
            src_verify[s["source_id"]] = {k: s.get(k) for k in ("rehash", "http_status", "fresh_sha256", "note")}
            src_verify[s["source_id"]]["verifier"] = v
        for st in doc["statements"]:
            st_verdict[st["statement_id"]] = dict(st, verifier=v)
        for sid in doc.get("skipped", []):
            st_verdict.setdefault(sid, {"verdict": "unverified", "verifier": v})
    for s in sources:
        if s.get("stored_text"):
            s["verify"] = src_verify.get(s["source_id"], {"rehash": "not_checked"})
    counts = {}
    for st in statements:
        v = st_verdict.get(st["statement_id"])
        if v is None:
            sys.exit(f"no verdict for {st['statement_id']}")
        verdict = v["verdict"]
        if verdict == "not_found":
            sys.exit(f"not_found verdict needs a drop rule: {st['statement_id']}")
        st["verdict"] = verdict
        st["verify_note"] = v.get("note")
        if verdict in ("wrong", "grade_wrong"):
            if st["statement_id"] not in CORRECTIONS:
                sys.exit(f"{verdict} with no typed correction: {st['statement_id']}")
            CORRECTIONS[st["statement_id"]](st)
        elif st["statement_id"] in CORRECTIONS:
            sys.exit(f"correction typed for a {verdict} statement: {st['statement_id']}")
        counts[verdict] = counts.get(verdict, 0) + 1
    json.dump(sources, open(os.path.join(G10, "sources.json"), "w"), indent=1, ensure_ascii=False)
    json.dump(statements, open(os.path.join(G10, "statements.json"), "w"), indent=1, ensure_ascii=False)
    print(f"sources {len(sources)} (stored {sum(1 for s in sources if s.get('stored_text'))}); "
          f"statements {len(statements)}; verdicts {counts}")


if __name__ == "__main__":
    main()
