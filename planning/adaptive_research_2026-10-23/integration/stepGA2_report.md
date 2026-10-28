# Step GA2: the CLI job door takes the project rule for `z_blend`

Implementer: I4. Date: 2026-10-28. Base: `5d343fc8d` (G-A). The lead's
item: `JobOp::operation_type` refused any key that the schema does not
publish. So after G-A, an old job file with `z_blend = false` refused as
"has no parameter z_blend", while a project file with the same key loaded.

Patch: `stepGA2_tree_vs_5d343fc8d.patch` (2 files). It leaves out I5's W2b
work in `session/compute.rs` and `generation.rs`.

## What changed

- Core `session/project_file.rs`:
  - New `pub enum RetiredParam { Refuse(&'static str), Drop }`.
  - New `pub fn retired_file_param(param, &toml::Value) -> Option<RetiredParam>`.
    It reads the one `RETIRED_VALUES` table; there is no second list.
  - A row whose value matches gives `Refuse(replacement)`.
  - Another boolean of a boolean row gives `Drop`. This is the loader's
    rule: the loader loads `false` and ignores the key.
- CLI `job.rs` `OperationDef::operation_type`:
  - First, a parameter whose `retired_file_param` is `Refuse` refuses with
    `"{key} = {value} is retired. {replacement}."`. That is the text of the
    parameter setter.
  - Then the unknown-key check skips a `Drop` key. The job reads its
    parameters by the schema names only (`job.rs`, the `params.defs`
    loop), so a dropped key reaches no op.
  - A string row (`clearing_strategy = "agent_search"`) now refuses at this
    check with the same text that the setter gave before.

## Line count

None; the step changes no GUI.

## Tests

New `--lib a_job_takes_the_project_rule_for_z_blend_ga2` (`rs_cam_cli`):

- `z_blend = false` passes, and the op type is Adaptive3d.
- `z_blend = true` refuses with "z_blend = true is retired", "Depth/Pass"
  and "Delete the key".
- An unknown key still refuses with "has no parameter".

## Gates (green)

- `cargo test -p rs_cam_cli`: 68 lib tests and every integration target
  passed.
- `cargo test -p rs_cam_core --test a_z_blend_true_refuses_at_load_g_a --test a_retired_clearing_strategy_is_named_r2`:
  green.
- `cargo clippy --no-deps -p rs_cam_core -p rs_cam_cli --lib --tests -- -D warnings`:
  clean.
- `cargo fmt --check`: clean on my files. The only diff is I5's untracked
  W2 test.
