# What "proven" means — acceptance spec

Written BEFORE the work, deliberately. The previous verdict failed partly
because the acceptance criterion was inherited from a test harness rather
than chosen, and it turned out to be unusable (sub-repeatability, aliased
by the measurement grid, and blind to a 28 mm uncut block). This is the
replacement, agreed up front.

**The claim under test (unchanged, §0.a):** on wanaka ×2, a cascade —
ball all-over finish (Op A) + ONE unified rest-clear (Op B) — beats **D**,
a single all-over pass with the tip tool, on wall-clock time without being
worse on the part.

---

## 0. Preconditions — the run does not count until these hold

The engine biases the comparison against the cascade today, and every one
of them scales with fragment count, which is Op B's defining trait
(12 780 fragments vs D's 1 045). Fix and verify first:

| # | defect | verified by |
|---|---|---|
| P1 | `ring_stepover` min-across-ring → scallop leaves ~837 mm² uncut | `v3_arc_direction_sanity`-style run shows **no `hit max_rings` warning** for either branch |
| P2 | `filter_air_cuts` classifies air from endpoints only | its own unit repro (a cut whose middle crosses material survives) |
| P3 | `refine_chord` never probes chords < 2×probe_step | `v3_chord_gouge_probe`: Op B `>0.5 mm` count within 2× of D's |

Already fixed and required to stay fixed: the `arcfit` reflex arc
(`v3_arc_direction_sanity` reports zero arcs whose arc-length/chord ratio
exceeds 10).

**Run hygiene, because this bit us:** ONE cargo process, verified by
`pgrep` before launch; a fresh log filename; and the run must be
reproducible — two invocations agreeing to <0.1 s, which our runs
currently do.

---

## 1. TIME — the claim itself

Both branches, **same run, same dials**, finish stack (Op A + Op B vs D).

- **PASS: cascade ≤ 0.95 × D.**

A margin, not "any win". Runs here are deterministic to 0.1 s so noise is
not the reason — 5% is the point below which the extra process complexity
(two ops, rest-region machinery, a second tool change) is not worth it.

---

## 2. THE PART — a panel, not one number

Three tests with different failure modes. Defects and coverage are HARD;
texture is comparative.

### 2a. Defects (hard — this is scrap risk)

- `rapid_collision_count` **= 0** on both branches.
- deep over-cut (columns < −0.5 mm) **≤ D's count**.
- worst over-cut **≤ D's worst + 0.10 mm**.

Rationale: a gouge is unrecoverable. Note D's own worst column is the
Rough's, so this is not a free pass — it is "the finish pass adds no new
gouging", which D's currently satisfies and the cascade's does not.

### 2b. Coverage (hard — this is "is the part finished")

- standing material, `>+0.5 mm` columns, **≤ D's count in every band**.
- **no `hit max_rings` warning** from either branch (P1).
- no connected uncut region **> 25 mm²** in either branch's deviation
  render.

The cascade already wins the first of these (1 301 vs 2 964). The third
is what the 28 mm block would have failed, and what no aggregate caught.

### 2c. Texture (comparative — and honestly bounded)

The ±10 µm bin is **retired**. It sits below machine repeatability
($11 junction deviation = 0.020 mm) and the 0.25 mm measurement grid
undersamples both branches' stepovers (0.21 and 0.363 mm), aliasing them
differently.

Replace with either:

- **p95 |dev| per band ≤ 1.1 × D's**, measured on a grid that resolves the
  finer stepover (**≤ 0.10 mm**) over a representative crop rather than
  the whole part, since a full-part 0.1 mm grid is ~4.8 M columns; or
- if that proves impractical, **drop texture from the gate entirely** and
  say so in the verdict. An unmeasurable criterion is worse than an
  absent one.

### 2d. Visual (hard — a human looks)

Surface + deviation renders for both branches, at full field and at a
20 mm crop. **Someone eyeballs them.** This is not ceremony: one look
found the uncut block, the facet-edge scribing and the moiré after six
sections of statistics had missed all three.

---

## 3. Evidence the process is what we claim

- Op B emits **Region spans** showing the strategy mix (§0.a's "with
  Region spans proving the mix"). Reported, not gated.
- Tool-load verdicts Within on both branches, or the exceedance is the
  shared Rough.

---

## 4. What a PASS licenses, and what it does not

A pass means: *on this fixture, at this cusp target, with this tool
pairing, the cascade is ≥5% faster and no worse on the part.* It does not
generalise to other terrain, and the fixture's coarse faceting (1.8% of
triangles carry 40.8% of the area) means fine-texture claims never
generalise from it at all.

## 5. What a FAIL licenses

A named verdict — which gate, by how much, and what would have to change.
Same as the current one, but earned on a clean engine instead of a
biased one.
