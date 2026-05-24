# MCP param UX overhaul — agent build prompt

Date: 2026-05-24

## Why this exists

An agent driving the live GUI via MCP today is half-blind whenever it touches
params. Two specific failure modes hit within five minutes of attempting the
agent smoke acceptance sweep
(`planning/SUGGEST_SIM_OPTIMIZE_AGENT_RUN_PROMPT.md`):

1. **No way to enumerate valid params per operation.** `get_toolpath_params`
   only shows currently-populated fields. Optional fields that serialize to
   `None` either don't appear at all or appear as a bare `null`, so an agent
   can't know they exist, what type they take, or what range is sensible.
   When the agent guesses (e.g. `prev_tool_radius` — a real `RestParams`
   field in core but not the MCP-layer param name), it gets
   `Error: Invalid parameter: unknown parameter '...' for X operation` —
   no list of valid alternatives.

2. **Mutations are write-only.** `set_toolpath_param`,
   `set_toolpath_enabled`, `set_tool_param`, dressup/boundary/stock setters,
   even `add_toolpath` return a one-line status string ("Set toolpath 0
   param 'X'. Regenerate to apply.") with no surfaced validation
   warnings, no applied-value echo (so silent clamps are invisible), no
   stale-flag deltas across other toolpaths, and no relay of GUI banners
   that the human user can see in the live app. The agent then proceeds
   into `generate_toolpath` / `run_simulation` and only finds out something
   was wrong from second-order diagnostics — or hangs because the GUI is
   showing an error the agent can't read.

We also tripped an immediate, surgical bug: `set_toolpath_param`'s wildcard
arm coerces JSON strings into `f64` and into `bool`, but not into integer
types, so any param backed by `usize` / `u32` / `Option<usize>`
(e.g. `prev_tool_id` on `RestParams`, plausibly several others) is
unreachable when an MCP client stringifies numerics — which the existing
`as_number` helper proves is a real, expected client behaviour.

## Goal

Make the MCP param surface usable for an autonomous agent. Specifically:

- An agent should be able to **discover** the full param surface of any
  operation kind without already knowing it.
- Every **mutation** should hand the agent back enough information to
  decide whether to continue, retry, or escalate — without a separate
  `get_project_diagnostics` round-trip per change.
- Numeric coercion should be **uniform** across `f64`, integer types, and
  `bool` so that an agent that only emits JSON numbers (or stringified
  numbers, as some MCP clients do) is not selectively blocked.

This is not a green-field rewrite. The existing `set_toolpath_param` /
`get_toolpath_params` shapes can stay; we are filling holes, not replacing
the surface.

## Scope split (build in this order)

Each block is intentionally PR-sized. Land them sequentially with passing
clippy + tests at each step.

### PR-1 — integer coercion gap (smallest, ~30 lines + tests)

**File:** `crates/rs_cam_core/src/session/compute.rs`, function
`set_toolpath_param`, the wildcard arm around lines 240–270.

**What to do:** extend the existing pattern that coerces JSON strings into
numeric `f64` so it also covers integer-typed existing fields. Concretely,
after the existing string→f64 coercion arm, add an arm that:

- If the existing field is a JSON `Number` with an integer value (or a
  `null` whose type the schema (PR-3 lands later) says is integer), and
  the incoming value is a `String`, parse the stripped string as `i64`
  first, and only fall back to `f64` if integer parsing fails.
- Same for incoming JSON `Number` values: round-trip via `as_i64` so a
  `1.0` arriving over the wire still deserializes into a `usize`-typed
  field rather than being rejected as "invalid type: float, expected
  usize".
- Cover `Option<usize>` / `Option<u32>` paths too — current behaviour is
  the same wildcard but the round-trip serde error message points at the
  outer Option, which is confusing.

**Tests:** add to the `set_toolpath_param_*` test family in
`crates/rs_cam_core/src/session/compute.rs`:

- `set_toolpath_param_prev_tool_id_accepts_int`
- `set_toolpath_param_prev_tool_id_accepts_string`
- `set_toolpath_param_prev_tool_id_accepts_null_to_clear` (Option clear)
- a regression test using `rest` operation specifically.

### PR-2 — `get_toolpath_params` exposes Optional/null fields with hints

**Files:**
- `crates/rs_cam_core/src/session/compute.rs` — wherever
  `get_toolpath_params` serializes the response.
- `crates/rs_cam_core/src/compute/catalog.rs` (or
  `compute/operation_configs.rs`) — `OperationConfig` variants.

**What to do:** currently the response strips `None` fields (default serde
behaviour). Instead, serialize the operation with `serialize_with` set so
that `None` fields appear as `null` keys, and annotate each field with its
serde-derived type in a sibling object. The simplest implementation:

- Add a per-variant `params_with_schema_hints()` method returning
  `serde_json::Map<String, ParamHint>` where `ParamHint` is
  `{ "type": "f64"|"usize"|"bool"|"enum:<variants>"|"option<...>", "required": bool, "default": Value }`.
- Implementations can use `schemars` if it's already a workspace
  dependency, or hand-roll for the ~22 op variants (one-time cost; the
  default values can be sourced from `OperationConfig::default_for_kind`
  which already exists implicitly via `add_toolpath`).
- `get_toolpath_params` returns the existing `operation.params` object
  AND a sibling `operation.param_schema` object keyed by field name.

**Don't** rename existing fields. Existing clients keep working; the new
`param_schema` field is additive.

**Tests:** `get_toolpath_params_includes_optional_null_fields` for each op
kind that has Optional fields (`rest`, `project_curve`, `drill`,
`adaptive`, `inlay`, anything else with `Option<T>`). Snapshot test fine.

### PR-3 — `get_operation_schema(operation_type)` MCP tool

**Files:**
- `crates/rs_cam_viz/src/mcp_server.rs` — new tool, alongside
  `add_toolpath` / `set_toolpath_param`.
- `crates/rs_cam_viz/src/mcp_bridge.rs` — request kind + response.
- `crates/rs_cam_core/src/session/compute.rs` — `pub fn
  operation_schema(operation_type: &str) -> Result<OperationSchema, _>`
  reuses the PR-2 `ParamHint` machinery without needing an active
  toolpath.

**Tool description (copy into the `#[tool(description = "...")]`
attribute):**

> Returns the full param schema for one operation kind without needing
> a toolpath: every settable field with its JSON type, whether it is
> Optional, the default value, and (when known) the valid range or enum
> variants. Use this BEFORE `add_toolpath` / `set_toolpath_param` when
> you are not already familiar with the operation's params. Supported
> `operation_type` values match `add_toolpath`.

**Response shape:**

```json
{
  "operation_type": "rest",
  "label": "Rest Machining",
  "params": [
    {
      "name": "angle",
      "type": "f64",
      "optional": false,
      "default": 0.0,
      "range": null,
      "description": null
    },
    {
      "name": "prev_tool_id",
      "type": "usize",
      "optional": true,
      "default": null,
      "range": null,
      "description": "Index of the prior (typically larger) tool used to define rest geometry"
    }
    // ...
  ],
  "tool_constraints": {
    "required_tool_type": ["end_mill", "bull_nose", "ball_nose"],
    "supports_v_bit": false
  }
}
```

The `description` and `range` fields can be empty for now — the structural
information is the load-bearing part. A follow-up can populate `range`
and `description` for the highest-traffic params.

**Tests:**
- `get_operation_schema_rest_lists_prev_tool_id`
- `get_operation_schema_unknown_op_returns_error`
- `get_operation_schema_drill_has_drill_specific_fields`
- A parametrised test that iterates every op kind in the catalog and
  asserts the schema's param set matches what
  `add_toolpath(kind).get_toolpath_params(0).operation.params` returns
  (with null fields included).

### PR-4 — mutation result envelope + GUI-banner relay

This is the biggest piece. It changes the return type of every mutation
MCP tool. Do this LAST, after PR-1/2/3 have landed, so we don't
re-litigate it twice.

**Affected mutation tools (in `mcp_server.rs`):**

- `set_toolpath_param`
- `set_tool_param`
- `set_toolpath_enabled`
- `add_toolpath`
- `remove_toolpath`
- `add_tool`
- `remove_tool`
- `set_boundary_config`
- `set_dressup_config`
- `set_dressup_field`
- `set_setup_face`
- `set_stock_config`
- `set_stock_source`
- `add_setup`
- `add_alignment_pin`
- `remove_alignment_pin`
- `move_toolpath_to_setup`

**New shared response shape** (define once in `mcp_bridge.rs` and reuse):

```rust
pub struct MutationResult<T> {
    pub ok: bool,
    pub summary: String,            // human-readable, what we already return today
    pub applied: T,                 // the value as actually written, post-coercion/clamp
    pub stale_toolpaths: Vec<usize>,// indices now stale (often the touched one + dependents)
    pub warnings: Vec<MutationWarning>,
    pub gui_banners: Vec<GuiBanner>,// any NEW banner the GUI surfaced during this mutation
    pub diagnostic_delta: Vec<DiagnosticEntry>, // new entries since last snapshot
}

pub struct MutationWarning {
    pub level: WarningLevel,        // info | warn | error
    pub field: Option<String>,      // which param triggered it, if scoped
    pub message: String,
    pub recommendation: Option<String>,
}

pub struct GuiBanner {
    pub kind: String,               // "validate" | "fix_banner" | "stale_default" | ...
    pub severity: String,
    pub title: String,
    pub detail: Option<String>,
}
```

**Where banners come from:** the GUI already produces them (e.g. the
"Validator Fix banner on Params tab" from commit `07a723d`, the
`stale_defaults` field already in `project_summary`). They live in the
`SimulationState` / project diagnostics layer. The mutation handler
needs to:

1. Snapshot the relevant diagnostic set BEFORE applying the mutation.
2. Apply the mutation.
3. Re-snapshot.
4. Diff and include any new entries in `diagnostic_delta` /
   `gui_banners`.

**Why this matters operationally:** today an agent that calls
`set_toolpath_param` and gets "ok" back has no idea whether the value was
clamped, whether it invalidated a downstream toolpath, or whether the
GUI is now showing the user a validator banner saying the value is
suspect. With this envelope, every mutation hands the agent enough state
to decide whether to continue, back out, or ask the user.

**Backwards compatibility:** keep existing one-line summary strings as
`result.summary` so anyone parsing the text output sees the same first
line; just emit JSON with the structured fields after. Or — cleaner — make
the change versioned: bump the MCP tool names to v2 (or add a `verbose`
flag) and keep the legacy path for a release. **Strong preference for
breaking change in one go** since this is internal tooling and the
project is pre-1.0.

**Tests:**

For each tool, add at least:

- `mutation_result_carries_applied_value_after_clamp` — set a stepover
  beyond machine envelope, assert the response shows the clamped value
  rather than silently saving the bad one.
- `mutation_result_emits_stale_dependents` — modify a tool diameter,
  assert every toolpath referencing it appears in `stale_toolpaths`.
- `mutation_result_relays_new_gui_banner` — trigger a validator banner
  (e.g. set DOC past machine rigidity) and assert it appears in
  `gui_banners` even when the mutation itself succeeded.
- `mutation_result_diagnostic_delta_is_delta_only` — pre-existing
  diagnostics MUST NOT reappear in the delta.

## Out of scope (do not touch in this branch)

- Don't add range/units metadata to every op param in PR-3 — leave
  `range` and `description` empty and let a follow-up PR fill them.
- Don't change the *underlying* validator logic — only relay what the
  GUI already produces.
- Don't redesign `get_diagnostics` / `get_project_diagnostics` /
  `get_toolpath_diagnostics`. Reuse them.
- Don't change `optimize_toolpath` / `run_simulation` return shapes;
  this PR is about mutation responses, not query responses.

## Acceptance checklist

A reviewer should be able to run, end-to-end:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test -q
```

with zero new warnings and zero new failures, AND:

```bash
cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp &
# in another shell or via Claude:
mcp__rs-cam__load_project test_data/ux_2d_pocket.toml
mcp__rs-cam__get_operation_schema rest
# → returns full param list including prev_tool_id with type usize, optional true
mcp__rs-cam__add_toolpath setup_index=0 operation_type=rest tool_index=1 model_id=1
mcp__rs-cam__set_toolpath_param index=1 param=prev_tool_id value=1
# → mutation result envelope shows applied: 1, ok: true, no warnings
mcp__rs-cam__set_toolpath_param index=1 param=stepover value=1000
# → mutation result envelope shows applied: <clamped value>, warnings: [...]
```

The agent smoke run (`planning/SUGGEST_SIM_OPTIMIZE_AGENT_RUN_PROMPT.md`)
should then complete cases AS006-AS018 without getting stuck on a
missing-param-name or a silently-rejected integer.

## Notes for the implementing agent

- Read `planning/toolpath_acceptance/baselines/2026-05-24_agent_smoke_acceptance.md`
  (will exist after Ricky asks me to write the partial-run baseline) to
  see exactly which params the live smoke run blew up on. The findings
  there motivate the schema-discovery requirement.
- The existing `set_toolpath_param` wildcard arm at
  `crates/rs_cam_core/src/session/compute.rs:240` is a great prior-art
  reference for how to handle JSON coercion without invasive plumbing.
- `crates/rs_cam_core/src/compute/catalog.rs` already enumerates the 22
  operation variants. Use that as the iteration source for the per-op
  schema endpoint — don't hand-list operation names in MCP code.
- `schemars` may or may not already be a workspace dep — if not, prefer
  hand-rolled `ParamHint` over adding a new heavy proc-macro dep.
- For PR-4, ground the design in
  `crates/rs_cam_viz/src/ui/properties/mod.rs` and `sim_diagnostics.rs`
  — those are where banners are currently rendered. Whatever GUI surface
  formats them today is the surface MCP should mirror.
- If the GUI banner state isn't already addressable from the MCP bridge,
  the right place to plumb it is the same channel `project_summary`
  uses for `stale_defaults` — a simple structured field on the shared
  session snapshot, not a new event bus.
