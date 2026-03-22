# Future Plans

## Benchmark Mode

### Status

Future phase. Not scheduled for implementation yet.

### Goal

Compare complete machining recipes, not just isolated toolpaths or single parameters.

Example comparisons:
- `adaptive rough + parallel finish` vs `adaptive rough + scallop finish`
- `coarse rough + fine finish` vs `fine rough + coarse finish`
- alternate tool diameters, stepovers, depths per pass, or operation orderings

### Product shape

Benchmark mode should be:
- CLI-first for authoring and execution
- artifact-driven for persistence and reproducibility
- GUI-second for browsing and comparing completed results

This keeps the main app from turning into a large parameter-matrix editor.

### Why CLI-first

The biggest UX risk is sweep definition. A GUI for:
- parameter grids
- per-operation overrides
- recipe branching
- ranking objectives
- constraints

would get cluttered quickly and be expensive to maintain.

The repo already has the right building blocks for a CLI-first design:
- declarative TOML job execution in `rs_cam_cli`
- simulation cutting metrics artifacts
- semantic traces and per-item cut summaries

### Recommended user flow

1. Define a benchmark manifest in TOML.
2. Run it from the CLI.
3. Generate multiple candidate recipes or trials.
4. Generate toolpaths and simulate each candidate.
5. Store per-trial artifacts and summaries.
6. Open the results in the GUI for sorting, filtering, and deep inspection.

### Scope of a benchmark trial

A trial should represent a full recipe:
- selected operations
- operation order
- tool assignments
- parameter overrides
- simulation metrics results
- pass/fail constraints

This is more useful than benchmarking a single toolpath in isolation.

### Core benchmark features

- named benchmark runs
- named variants / recipes
- parameter sweeps over selected operation fields
- sequence variants:
  - enable or disable operations
  - swap operation type
  - reorder operations
  - swap tool
- constraints:
  - no generation failure
  - no collisions
  - no holder or shank collisions
  - peak chipload below threshold
  - peak axial DOC below threshold
  - low-engagement time below threshold
- objectives:
  - lowest total runtime
  - lowest cutting runtime
  - lowest air-cut time
  - lowest low-engagement time
  - best removed-volume rate
  - weighted composite score
- resume and retry support
- concurrency cap
- stable artifact output per trial

### Results that matter

At minimum, each trial should store:
- config snapshot
- generated toolpaths
- generation traces
- simulation cut trace
- per-toolpath summaries
- per-semantic-item summaries
- failure reasons
- warnings and constraint violations
- aggregate score and rank

### GUI role

The GUI should act as a results browser, not a sweep editor.

Good GUI responsibilities:
- list completed trials
- sort and filter by metrics
- group by parameter or recipe
- compare selected trials side-by-side
- open one selected trial in the Simulation debugger
- show deltas versus a baseline

Avoid in v1:
- building large parameter matrices in the GUI
- authoring full benchmark manifests in the GUI
- embedded search-strategy configuration UIs

### Recommended GUI surfaces

- a lightweight benchmark results table
- per-trial summary cards
- cross-run charts:
  - total, cutting, rapid, and air-cut time
  - engagement
  - MRR
  - chipload
  - worst semantic items
- baseline delta view
- `Inspect in Simulation` for a selected trial

### Quality caveat

Time-only comparison is not enough for finish strategy comparisons.

For examples like `parallel` vs `scallop`, benchmark mode should either:
- require equal quality-driving settings, or
- include a future finish-quality metric before treating one result as better

Until then, finish-strategy benchmarking should be treated as constrained comparison, not pure time optimization.

### Recommended architecture

Primary implementation target:
- a new `rs_cam_cli` benchmark subcommand

Suggested command shape:
- `rs_cam benchmark run benchmark.toml`
- `rs_cam benchmark resume <results-dir>`
- `rs_cam benchmark export <results-dir> --csv`

Suggested manifest concepts:
- benchmark metadata
- baseline recipe
- variants
- sweep definitions
- ranking objective
- pass/fail constraints
- concurrency limit

### Example benchmark scenarios

- roughing strategy comparison
- roughing + finishing sequence comparison
- tool diameter sweep for the same recipe
- stepover / depth-per-pass sweep
- smaller rough tool vs more detailed finish tradeoff
- alternate finishing strategy for the same quality target

### Phased implementation

#### Phase 1

CLI benchmark manifest and trial runner.

#### Phase 2

Artifact format and ranked summary output.

#### Phase 3

GUI results browser and side-by-side compare view.

#### Phase 4

Open selected trial directly in the Simulation debugger.

#### Phase 5

Smarter search and benchmark analytics:
- Pareto front view
- parameter importance
- auto-pruning of invalid candidates

### Out of scope for v1

- Bayesian optimization
- force, power, or tool-load modeling
- batch authoring UI in the main app
- benchmark config stored in normal project files
- cross-run physical surface-quality prediction

### Follow-on after benchmark mode

If benchmark mode lands well, the natural later extension is:
- quality-aware objective functions
- finish prediction
- cutting force / tool-load estimation
- agent-driven benchmark search on top of the same artifact format
