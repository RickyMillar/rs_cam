# F-018 — `test_data/ux_*.toml` templates don't match smoke CSV

- **Stage:** infra (test fixtures)
- **Severity:** medium
- **Status:** open
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (~regenerate from CSV)
- **Linked PRs:** —
- **Source audits:** smoke-run evidence

## Evidence

`planning/toolpath_acceptance/cases_agent_smoke.csv` specifies a
tool and material per case, but the templates in `test_data/` don't
always provide them:

- **AS002 (adaptive 2D)** wants softwood; `ux_2d_pocket.toml` stock
  is hardwood
- **AS004 (face)** wants MDF; template is hardwood
- **AS005 (zigzag)** wants MDF; template is hardwood
- **AS007 (trace)** wants 3mm end mill; `ux_2d_star.toml` has only
  6mm + 12.7mm V-bit
- **AS009 (chamfer)** wants V-bit 6mm; template has only V-bit 12.7mm
- **AS011 (drill)** wants softwood; template hardwood
- **AS013 (adaptive3d)** wants softwood; template hardwood
- **AS018 (project_curve)** needs both a source curve and a surface
  STL; `ux_3d_terrain.toml` only has the surface
- **All templates with `ux_3d_terrain.toml`** carry a broken
  pre-existing toolpath `Rivers (back) (copy)` that errors on every
  load and sim ("Selected model is missing")

## Acceptance test

1. **Equivalence**: for each row in `cases_agent_smoke.csv`, the
   referenced template's `tools` and `stock.material` must match
   the CSV's `tool_name` / `material_family` (within the family
   group — Generic Hardwood satisfies hardwood family).
2. **Cleanliness**: `cargo test --test param_sweep` after this fix
   doesn't fail because a template is broken (it doesn't today, but
   add a smoke test that loads each template and asserts no errors).
3. **Smoke verification**: re-running cases AS002/4/5/7/9/11/13/18
   uses the correct tools and materials from the template, without
   the agent having to add tools mid-run.

## Files

- `test_data/ux_2d_pocket.toml` — needs softwood + 3mm tool variants;
  consider splitting into `ux_2d_pocket_hardwood.toml` and
  `ux_2d_pocket_softwood.toml` rather than one polymorphic template
- `test_data/ux_2d_star.toml` — needs 3mm and 6mm V-bit
- `test_data/ux_step_plate.toml` — needs MDF stock variant
- `test_data/ux_3d_terrain.toml` — remove broken `Rivers (back) (copy)`
  toolpath; add a source curve for project_curve
- `test_data/ux_step_stepped.toml` — verify MDF variant
- `test_data/ux_step_lbracket.toml`, `ux_step_block.toml` — verify
  smoke CSV doesn't need anything from these

## Fix shape

Three options:

**A)** Add per-material / per-tool template variants
(`ux_2d_pocket_hardwood.toml`, `ux_2d_pocket_softwood.toml`,
`ux_2d_pocket_mdf.toml`, etc.) and update the smoke CSV to point at
the right one per case.

**B)** Make the smoke runner mutate stock material and add tools
on-the-fly via MCP. This is what the current agent had to do as a
workaround.

**C)** Add a `--material` / `--tool` flag to `load_project` MCP that
overrides the template's defaults.

**Recommend A** — fixture files are cheap, the runner stays simple,
and the resulting matrix is more self-evident.

## Risk

S. Pure fixture work. Adding files doesn't break anything.

## Notes

- Also update `cases_agent_smoke.csv`'s `project_template` column to
  point at the new variant filenames.
- **Defensive note**: do not delete the existing templates. They may
  be referenced from other docs / tests.
- The broken `Rivers (back) (copy)` toolpath in `ux_3d_terrain.toml`
  is a separate bug — it should not load at all, or should load with
  a warning. Investigate alongside F-004 (project loaders).
