# C21: read one op's section of a Phase 0 .nc file into model.py moves.
# GRBL modal G0/G1/G2/G3 and F; I/J relative to the start point.
import re

WORD = re.compile(r"([GXYZIJF])(-?[\d.]+)")


def section(path, title):
    """The moves of the op whose comment is `(title)`, up to the next
    comment line that starts another op or a setup. Returns (seed, moves):
    `seed` is the tool position before the first move of the section."""
    pos = [0.0, 0.0, 0.0]
    mode, f = "G0", 0.0
    seed, moves, inside = None, [], False
    for line in open(path):
        s = line.strip()
        if s.startswith("("):
            if s == f"({title})":
                inside = True
            elif inside and not s.startswith("(feeds"):
                break
            continue
        words = dict()
        for k, v in WORD.findall(s):
            if k == "G":
                g = "G" + str(int(float(v)))
                if g in ("G0", "G1", "G2", "G3"):
                    mode = g
                continue
            words[k] = float(v)
        if not any(k in words for k in "XYZ"):
            if "F" in words:
                f = words["F"]
            continue
        if "F" in words:
            f = words["F"]
        tgt = (words.get("X", pos[0]), words.get("Y", pos[1]), words.get("Z", pos[2]))
        if inside:
            if seed is None:
                seed = tuple(pos)
            kind = {"G0": "rapid", "G1": "line", "G2": "cw", "G3": "ccw"}[mode]
            moves.append((kind, tgt, f, words.get("I", 0.0), words.get("J", 0.0), kind))
        pos = list(tgt)
    return seed, moves
