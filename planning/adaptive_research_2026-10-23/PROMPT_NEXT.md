# Prompt for the next session (lead, any account) — 2026-10-24

You are the LEAD of the adaptive roughing research. Orchestrate agents;
do not read, code or run fixtures yourself. Read first, in this order:

1. `STATUS.md` — "Where things stand" at the top, then the agent table.
2. `PLAN.md` (rulings, fixtures, look protocol, gates) and
   `PROMPT_LEAD_2026-10-23.md` (how agents are run; still binding).
3. `DEFECTS.md` (14 shipped-code findings), `P15_D_verify.md` (D2–D8:
   every gate result), `P2_C2_geodesic_fixes.md` (C2–C8: every join idea
   tried, with measurements), `P15_E_evidence.md` (E1–E4: the instruments
   on the Corne and wanaka200 controls).

## State (2026-10-24 01:15 NZDT)

- Everything is on master (609ce75b + planning commits), gates green.
  Branch `adaptive-instruments` == master; worktree
  `.claude/worktrees/adaptive-head` (own target, ~12 GB) is where agents
  edit; delete it with its target when the research ends.
- The installed GUI `target/release/rs_cam_gui` is the branch build
  (sha 8820c48c…, mtime 1792752676.96); `gui_status` before trusting it.
- Peer session (geometry-selection lead, name changes per session) works
  in the main checkout; coordinate every `--release` build and ff by
  SendMessage. Their R2/Q7/Q9 landed as e5362911 (2026-10-24 01:30):
  `ToolpathConfig::tool_id` is now `Option<ToolId>` (six
  tests/adaptive3d_*.rs literals took `Some(..)`, no behaviour change);
  `effects_are_stamped_wp19` MAX_EXCLUDED_FILES 40 -> 41 for the A2
  controller test. Master is e5362911 or later; the worktree branch is
  behind it — `git merge master` there before any agent edits.
- Instruments (Phase 1.5) are DONE: rough gouge rule (Allowance) and rough
  coverage (plan's own reachable floor, islands with `touches_wall`),
  both read what the eye saw on the controls (P15_E_evidence.md E3/E4).
- Candidate (a), the geodesic wavefront: G1 passed (operator: "the growth
  pattern looks like the classic adaptive I wanted"). State to show:
  `g1_2d_renders_v4.html` (`geodesic_curv`: star 73 entries, pocket 63).
  Serve the package dir with `python3 -m http.server 8766` (the
  operator's browser is a Flatpak and cannot read repo files directly).

## Already in flight when this was written

A lead session named rs-cam-e3 picked up this handoff at about 01:30 on
2026-10-24 and started C9 (far-side re-seed, column `geodesic_curv_reseed`)
and C10 (the lead-in guard, read-only); its rows are in STATUS.md. Before
you brief anything, check STATUS.md, the worktree branch log and
`P2_C2_geodesic_fixes.md` / `P2_C10_leadin_guard.md` for their results, and
look for a running session with ListAgents, so two leads never drive the
same worktree.

## The two open problems on the 2D shapes (fix these before 3D)

1. **The far side of the island (meeting line).** The two front branches
   meet behind the island; the spacing law uses ρ from the entry, so the
   wraps there are too far apart, one is refused and the refusal cascades
   (planner stamps only accepted cuts). Operator's two ideas (2026-10-24,
   01:20), both sound, test them in this order:
   a. **Let the first-arriving branch keep diffusing.** After the
      one-side rule, re-seed the field from the CLEARED area (T = 0 on
      every cell the accepted passes stamped, the island and walls as
      obstacles) and let that single front fill the far side; the
      second branch then fills only the gap it left. This is FreeCAD's
      "grow from the cleared area" (R1 §2) in field form, and the
      principled fix. No new constant.
   b. **Split at the meeting line** into two regions and run the
      spaghetti zone as its own pass. Fallback if (a) cascades.
   Also keep `curv_local` (spacing from the front's own centre of
   curvature) in mind: it removed the cascade (876 → 202) but packs wraps
   at the island corner; it needs a DERIVED spacing floor at the bend
   node, never a tuned one.
2. **The per-arc plunge in lobes.** 58 of 73 star entries are wrap-end
   joins. Tried and refuted under the swept-width rule: ramp (C3), morph
   (C4/C5), transition (C6), the exact-area measure (C7 — same answer on
   circular fronts and at wall ends), mid-arc lead-in (C8 — the code
   never applied: 0 lead-ins, every band fell back; FIND THAT GUARD
   first, it may be a bug not a limit). The one unexplored, derived
   option: the lead-in bite grows along the arc so the swept width stays
   ≤ c·D — C8's radius bound (23–26 mm) says it needs long arcs; measure
   where it fits before more design.

## Then

- Wire candidate (a) as a `ClearingStrategy3d` arm (the 2D kernel runs
  per Z slice as the others do), run rivmap100, Corne and wanaka200 with
  the hard bars (now instrumented), make the fixture pages (protocol in
  PLAN.md), G2.
- Shipped-code defects to schedule (DEFECTS.md): G-MOPLOAD (the residue
  mop breaks the pass-load invariant; the V1 mechanism), G-RESLINKLOAD,
  G-CELLREAD (the planner cell under-reads), G-SHELFSCAN (115 m of rapid
  on the wanaka back rough), G-WALLSTRIP-PLAN, G-LINKPLOUGH.

## Rules that bit us tonight

- Every candidate constant derives from R, s, tolerance, cell, ceiling;
  the verifier scans every diff for bare floats.
- An agent's "predicted" number is not a result; D-runs decide. Four
  join ideas looked right on paper and failed in the run.
- Pre-register the keep-or-drop rule before the run (C5 did; it worked).
- Agents never commit; the lead commits by pathspec; peers' dirty files
  block a ff — merge with theirs as base.
