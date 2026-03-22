# Review: CLI Commands & Job Runner

## Scope
The batch CLI: 13 direct commands and the TOML job runner.

## Files to examine
- `crates/rs_cam_cli/src/main.rs` (Clap definitions)
- `crates/rs_cam_cli/src/job.rs` (TOML runner)
- `crates/rs_cam_cli/src/helpers.rs`
- `fixtures/demo_job.toml`
- Any CLI tests

## What to review

### Command surface
- All 13 commands: are they documented? (--help output)
- Parameter naming: consistent across commands?
- Required vs optional parameters: sensible defaults?
- Error messages: user-friendly?

### Job runner
- TOML schema: documented? validated?
- Multi-setup support in job files
- Operation sequencing within a job
- Output file naming for multi-output jobs

### CLI vs GUI parity
- Are there GUI features not accessible from CLI?
- Are there CLI features not in GUI?
- Parameter names: same terminology?

### Edge cases
- Invalid input files
- Missing required parameters
- Output file already exists (overwrite? error?)
- Very large jobs

### Testing
- Are there CLI integration tests?
- Demo job: does it still work?

## Output
Write findings to `review/results/39_cli.md`.
