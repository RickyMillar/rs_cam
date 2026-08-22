# Live-GUI session plan — 2026-08-22, post lateral campaign

Binary: `cargo run --release -p rs_cam_viz --bin rs_cam_gui -- --mcp` off the
pre-built release (built at campaign close — do NOT let the MCP connect race a
cold compile; if in doubt `cargo build --release -p rs_cam_viz --bin
rs_cam_gui` first and wait).

Context: the lateral-setups campaign closed at `fccd5968` (see
`planning/lateral_setups_2026-08-22/SPEC.md` — every §3 item landed, closed
unreachable, or blocked-with-evidence). Everything below needs a LIVE GUI and
is ordered by value.

## 1. Ø1 tapered-ball chipload `Within` — the open verdict question

F3000 on the Ø1 tapered ball reads chipload `Within` and must NOT be trusted
as clearance to cut. Hypothesis (UNTESTED): the gate reads ACHIEVED advance
from the kinematics-predicted feed while the pre-sim heuristic reads
COMMANDED feed — both may be right about different quantities.

Steps: load the project that carries the Ø1 tapered ball → `run_simulation` →
`get_tool_load_report().per_toolpath[]` for that toolpath. Read, in order:
- the chipload gate's POPULATION first (`sample_count` / `sample_range`) — a
  gate on an empty population passes vacuously (repo law, 4+ occurrences);
- the observation basis: chipload observes ADVANCE PER TOOTH =
  `effective_feed ÷ (rpm · flutes)` — check which feed `effective_feed` is;
- `band_capped_from` shape if a floor clamp is involved (`Some(0.025)` = band
  found; `None` = no vendor row matched — the Ø1 case is a NO-ROW case, P1's
  envelope fallback deliberately does not reach it).
Probe the code path you are accusing: the gate is the ENVELOPE resolver, the
pre-sim Suggest is the RECIPE resolver — two wrong diagnoses in one day came
from conflating them (memory: feedback_probe_the_same_code_path).

## 2. First human eyes on a lateral setup — nobody has ever SEEN one

The scrub fix is sentried on Z-grid solid volume; no human has watched a
Front-setup replay. Build a throwaway lateral demo via MCP:
- load any mesh project → `add_setup` → `set_setup_face` Front → one 2D op
  (pocket, drawing authored in the work plane) + one drill → `generate_all`
  (needs `simulation_resolution_mm` if rest ops) → `run_simulation`.
- `sim_scrub_toolpath` through the lateral group; `screenshot_simulation` at
  several positions and Read the images. The groove/hole must be VISIBLE in
  the solid (pre-fix it was 0.0 mm³ / invisible).
- Check checkpoint vs live-scrub agreement visually; `screenshot_simulation`
  on a local-framed checkpoint exercises the new mesh-route branch in
  `app/mcp.rs`.
- Known deliberate behaviour: inside a lateral group, earlier setups' cuts
  are NOT composited. Confirm it reads as sane, not broken.
- Refusal UX: try (a) a lateral setup on a 2D-only project, (b) a lateral
  setup with an enabled fixture. Both must refuse with the typed messages
  naming the workaround — check they surface legibly in the GUI (notification
  + failed-submit), not just in a log.

## 3. Composite renderer eyeball (TD3 leftover)

TD3 fixed EVERY panel of the 6-view composite being mirrored. It has had a
partial line-by-line review only — one human look at the composite PNGs
(sweep output or `screenshot_*`) against a known-chirality part closes it.

## 4. If at the physical machine (optional)

Live wanaka re-measure: fresh wall-clock vs predicted cycle time for the ±10%
machine-profile closure (memory: project_machine_profile_gui). Sim-side
predictions changed since the last measurement (G-DRILLTIME folded drill
runtimes into totals).

## Known-not-fixed, do not re-file
- GUI chipload heat-map colours by the OLD per-move quantity (ledgered).
- G-LATERALKEEPOUT: lateral + enabled fixture refuses by design.
- G-GEOMCACHE-FLAKE / G-VIZARTIFACT-FLAKE: test-only, filed in RUN_LOG.
- SimGroupEntry.direction stays: S5 cache-key reader + peer-owned bench
  producer (SPEC §2e).

## Standing constraints (unchanged)
Never touch `.mcp.json`, `planning/airrun_2026-06-01/wanaka.toml`, workspace
`Cargo.toml`, `benches/hot_paths.rs`, `tests/perf_golden_*`,
`planning/perf_review_2026-08-19/`. One cargo job at a time (`pgrep -x
cargo` + `/proc/<pid>/cwd`, never `pgrep -f`). No history rewriting; explicit
staging only. Don't pipe `cargo test` through `head`.
