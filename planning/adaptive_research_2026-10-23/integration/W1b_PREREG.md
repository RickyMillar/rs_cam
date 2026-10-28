# W1b pre-registration (old projects load helical)

Written before the code, on HEAD 2881bab01 (W1 = a884902e6).

## Mechanism

- The project format stays 3. `check_format_version` refuses every other version, and its doc forbids a converter
  ("a second reader"), so a version bump would refuse every old file. Not used.
- The key is renamed: the Waterline dial is `helical` in the file, the schema and the code (`WaterlineConfig::helical`,
  serde default `true`). A file with no `helical` key loads `true`.
- The old key `continuous` on a `kind = "waterline"` op is read on the raw table in the one text door
  (`ProjectFile::from_toml_str`), in the same walk as `RETIRED_OP_ENTRIES`. The loader does not refuse it (the
  operator wants the file to load). It does not ignore it in silence either (the `RETIRED_KEYS` rule: "a key that the
  loader ignores hides an edit"): each hit gives a `ProjectLoadWarning::WaterlineHelical` in the existing load
  warning channel, as `DressupsNormalized` does for its one-shot migration.
- A file saved after W1b writes `helical = <value>`, so an explicit `false` stays.

## Fixtures and goldens that store a Waterline with `continuous`

Search: `rg -l 'kind = "waterline"'` over `crates/` and `planning/fixtures/`: no project file. The op golden and the
perf golden build their ops in code. So no fixture or golden changes; nothing is re-blessed.

Out of the gate (not fixtures): `planning/adaptive_research_2026-10-23/phase3/d69/jobs/*.toml` (the play copies) hold
`continuous = false` and will load helical with one warning per Waterline.

## Code that names the field (rename only, no behaviour change)

`crates/rs_cam_core/tests/perf_golden_sim_metrics.rs:593` (`continuous: false` → `helical: false`, golden unchanged),
the W1 sentry and the two W1 test pins, `compute/execute/finish_raster.rs`, and
`crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:265` (`cfg.continuous` → `cfg.helical`; I4's file, one
token, needed to compile).

## Sentries

1. An old file (`continuous = false`) loads `helical = true` and gives one warning.
2. A new file with `helical = false` loads `false`, with no warning.
3. Save, load, save keeps `helical = false` and `helical = true`.
