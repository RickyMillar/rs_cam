# P29 investigation plan: one cut object, every limit and metric a function of it

Written by the lead on 2026-11-11, before a context compaction. The next step is INVESTIGATION
ONLY: no product code. The output is a change list and a design the operator approves. The
session compacts again before implementation.

## The direction (operator, 2026-11-11)

> "could we not have it so every 'cutting move' is an object that must implement force and
> similar? [...] Seems like we need an architectural fix for all of this"
> "will this make graphing and displaying the data more uniform too? is there anything else we
> should pull into this. other metrics that might have similar issue"

Cause: `phases/P28_limit_audit.md` found 34 places where an op family, pass role, strategy or span
kind is exempt from a limit (9 physical, 11 scope, 6 rule of thumb, 7 unknown; 3 hold an operator
ruling). The worst: the Adaptive family skips the machine force budget (P13c scope decision, never
brought to the operator). Each limit decides its own scope in its own `match`.

The target architecture, as put to the operator (they agreed with the shape):

1. A limit sees only a cut (an engagement), the tool, the stock and the machine. Never the op type.
2. Every op declares all the cuts it makes (body pass, first or slot pass, entry, cleanup) through
   a required method; a new op does not compile without it. The op type decides which cuts exist,
   not which limits apply.
3. Every cut goes through every limit: Suggest (the largest cut where all declared cuts pass), the
   post-simulation gates (every sample that removes material, no span filter), the modulator.
4. "Does not apply" comes only from the cut's own geometry (a drill has no radial engagement).
5. Every limit and metric returns ONE record shape (value, limit, margin, binding, basis:
   sourced / calculated by a rule / no number), so cards, charts and MCP read one thing, and
   planned cuts and simulated samples plot as the same kind of point.

## What to investigate (in this order; write findings to `phases/P29_FINDINGS.md`)

1. **What exists.** The engagement type and its one door (`feeds/engagement.rs`,
   `Engagement::from_sample`, K7/P15), `CutSpec` / `FirstPass` / `PowerCases` (`compute/catalog/`),
   the op registry (`OpRegistryEntry`, `every_op_conforms_w4`), the span and move kinds
   (`SpanKind`, `MoveIntent`, `is_steady_state_for_gate`), the verdict types (`ChiploadVerdict`,
   `PowerVerdict`, `DeflectionVerdict`, the depth verdict), the envelope limits (`LimitKind`,
   `SafeEnvelope`), the modulator's constraint set. For each: what it holds, who reads it, how far
   it is from the target.
2. **The planned-cut list per op.** For every operation kind in the registry: the cuts it really
   makes (from its generator), which of them it declares today, which are missing (entry, slot
   ring, cleanup sweep, the cut out of a helix, finishing step-down). One table, every op.
3. **The limit list.** Every limit and check as a signature: what it needs from a cut, and every
   place it reads the op family, the pass role, the strategy or the span kind today (start from the
   34 rows of P28; confirm each in code; add what P28 missed). Mark which become a pure function of
   the cut at once and which need a new field on the cut.
4. **Metrics beyond limits** (the operator's second question). Audit anything computed per op type
   or behind a span filter: the entry rules of each emitter (ramp angle, helix radius, plunge
   limit; P22b showed the 3D and 2D emitters differ), the pre-simulation tool-body checks, the
   vendor depth table and axial bound, the minimum chip, efficiency and air-cut figures, the
   reached feed share, finish quality measures (scallop, tip float), Suggest's width and depth
   proposals, the provenance label of each value. Same table form as P28.
5. **The display side.** Every surface that draws a load or limit value (the four sim cards, the
   width x depth chart, the envelope plot, the nomogram, the Suggest card, the lineage, MCP
   `get_tool_load_report`, `get_suggest_rationale`, `get_safe_envelope`, the CLI): what shape it
   reads today and what one record would replace.
6. **What moves.** For the benchmark (wanaka, both the operator file and
   `nz_wanaka_21_6mm_rough_work.toml`) and FM1: an estimate, by reading and by small probes where
   cheap, of which ops change depth, width, feed or verdict when every cut reads every limit. Name
   the limits that would start to bind (the 35.3 N hobby force budget first) and how much each
   matters. Flag any limit whose own basis is weak enough that making it universal is a risk.
7. **The change list.** Ordered phases, smallest safe step first, each with: what it changes, its
   sentry, its predicted pin moves, the operator ruling it needs. Step one changes no number (the
   structure with today's behaviour). The Adaptive force budget is the first behaviour step.
8. **Fit with the other open designs.** `P25_DESIGN.md` (row lookup as categories; rulings 1 to 5
   given), `P27_BRIEF.md` (feeds from fundamentals, for a dedicated session), `P26_survey.md` (no
   width rule for adaptive ops), `P24b_survey.md` (one force line stays). Say whether P29 comes
   before P27, inside it, or beside it, and what P27's brief must say about P29.

## How to run it

- One or two read-only agents at most (normal token use); the lead reads the findings and writes
  the design. Agents never commit. Probes are temporary tests through `scripts/cargo_lane.sh`
  (`-j 2`, in the foreground, deleted after). No heavy suite while the GUI holds a simulation.
- Bring to the operator: the planned-cut table, the limit list with every op-type read, the
  metrics audit, the estimate of what moves, the phase list. Anything that leaves a toolpath out
  of a limit is the operator's decision (memory: surface scope deferrals).
- Then compact, then implement.

## State at the time of writing (see `NEXT_SESSION.md` for the detail)

- HEAD has P21c, P22b, P23a to P23d, P24a; the wanaka acceptance table is PASS 98 / FAIL 0 on
  `6f586706c` (it grades Suggest's numbers, not the job).
- The operator chose to rough with the 6 mm tool. Working file
  `planning/rivmap_block_job/nz_wanaka_21_6mm_rough_work.toml` (stepover 1.2 mm from the add door;
  Suggest's Pocket rule gives 5.12 x 3.0 mm; the operator has not confirmed a stepover; idx 15 is
  to be deleted; the scallop finish is not simulated on the new stock: it needs free memory, and
  the operator offered to help clear it).
- Open rulings: P25 ruling 6 (no row: treated as "no number"); the stepover above.
- The GUI restarts through the MCP proxy (`gui_restart`; check `gui_status`). Agents alive at the
  time of writing: `p21c-impl` (implementer, feeds context), `compare-runner` (MCP runs).
