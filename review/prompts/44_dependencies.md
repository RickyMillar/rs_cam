# Review: Dependency Health

## Scope
All external crate dependencies — versions, maintenance, alternatives.

## Files to examine
- `Cargo.toml` (workspace)
- `crates/rs_cam_core/Cargo.toml`
- `crates/rs_cam_cli/Cargo.toml`
- `crates/rs_cam_viz/Cargo.toml`
- `Cargo.lock` (actual resolved versions)

## What to review

### Per-dependency audit
For each external crate:
- Current version vs latest available
- Maintenance status (last release date, open issues)
- Is it the best choice for this use case?
- Any known vulnerabilities?

### Key dependencies to focus on
- `nalgebra 0.33` — math foundation
- `stl_io 0.11` — only STL parser
- `kiddo 4` — spatial indexing (alternatives: rstar?)
- `cavalier_contours 0.7` — polygon offsets (niche crate)
- `usvg 0.47` — SVG parsing (resvg ecosystem)
- `dxf 0.6` — DXF parsing
- `eframe/egui 0.30` — GUI framework
- `geo 0.32` — polygon operations

### Concerns
- Any dependencies that are unmaintained or have < 100 downloads?
- Version pinning strategy: exact, caret, or tilde?
- Feature flags: are unused features enabled?
- Dependency tree depth: any surprising transitive dependencies?

### Build
- Compile time impact of each dependency
- `cargo audit` output (if runnable)

## Output
Write findings to `review/results/44_dependencies.md` with a table: Crate | Version | Latest | Status | Concern.
