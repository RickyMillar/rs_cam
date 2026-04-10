# Parameter Validation Warnings — Based on Feeds/Speeds/Loading Calculations

## Context

The GUI allows users to set toolpath parameters (stepover, depth per pass, feed
rate, etc.) to any value without warning when they significantly exceed the machine
and tool capabilities computed by the feeds and speeds system. The existing
auto-calculated values from the machine/tool/material model should serve as the
reference — warn when user overrides deviate dangerously.

## What we have

The feeds and speeds system already computes recommended values based on:
- Machine profile: spindle range, power, rigidity factors
- Tool: diameter, flute count, material, geometry
- Material: hardness index, specific cutting force (Kc)
- Operation type: roughing vs finishing factors

These are exposed via `FeedsAutoMode` on each toolpath config, which tracks which
parameters are auto-calculated vs user-overridden. The auto values come from the
rigidity profile in `MachineProfile`:

```
rigidity.doc_roughing_factor    → depth_per_pass for roughing
rigidity.doc_finishing_factor   → depth_per_pass for finishing
rigidity.woc_roughing_factor    → stepover for roughing (fraction of diameter)
rigidity.woc_roughing_max_mm    → stepover cap
rigidity.woc_finishing_mm       → stepover for finishing
rigidity.adaptive_doc_factor    → adaptive depth multiplier
rigidity.adaptive_woc_factor    → adaptive width multiplier
```

## Warnings to implement

### Stepover warnings

| Condition | Severity | Message |
|-----------|---------|---------|
| stepover > tool_diameter | Error | "Stepover ({val}mm) exceeds tool diameter ({d}mm) — will leave uncut strips" |
| stepover > tool_diameter * 0.8 | Warning | "Stepover is {pct}% of tool diameter — may leave visible scallops" |
| stepover > recommended * 2.0 | Warning | "Stepover ({val}mm) is {mult}x the recommended value ({rec}mm) for this machine/material" |
| stepover < tool_diameter * 0.05 | Info | "Very fine stepover — cycle time will be significantly longer" |

### Depth per pass warnings

| Condition | Severity | Message |
|-----------|---------|---------|
| depth > tool_cutting_length | Error | "Depth ({val}mm) exceeds tool cutting length ({cl}mm)" |
| depth > recommended * 2.0 | Warning | "Depth ({val}mm) is {mult}x the recommended value for this machine rigidity" |
| depth > tool_diameter * 1.5 | Warning | "Depth exceeds 1.5x tool diameter — high deflection risk" |

### Feed rate warnings

| Condition | Severity | Message |
|-----------|---------|---------|
| feed > machine.max_feed_mm_min | Error | "Feed rate ({val}) exceeds machine maximum ({max} mm/min)" |
| feed > recommended * 2.0 | Warning | "Feed rate is {mult}x the auto-calculated value — risk of tool breakage" |
| feed < recommended * 0.2 | Info | "Feed rate is very low — may cause rubbing and heat buildup" |

### Tool-specific warnings

| Condition | Severity | Message |
|-----------|---------|---------|
| Ball nose used for pocket/adaptive (2.5D) | Info | "Ball nose tools leave cusps on flat floors — consider an end mill" |
| End mill used for scallop | Error | "Scallop requires a ball-tip tool" (already enforced) |
| Tool shank > machine.max_shank_mm | Warning | "Tool shank ({s}mm) exceeds machine collet capacity ({max}mm)" |
| Stepover > tool_radius for ball nose finish | Warning | "Scallop height will exceed {h}mm at this stepover" |

### Operation-specific warnings

| Condition | Severity | Message |
|-----------|---------|---------|
| adaptive3d.stock_top_z > stock.z * 2 | Warning | "stock_top_z ({val}mm) is much higher than stock ({sz}mm) — excessive air cutting" |
| drop_cutter.min_z > model.bbox.max.z | Warning | "min_z ({val}mm) is above the model — no material will be cut" |
| horizontal_finish on model with < 10% flat area | Info | "Model has few flat areas — horizontal finish will produce minimal cuts" |

## Implementation approach

### Where to add warnings

**Option A: Validation at set time (recommended)**
- In `set_toolpath_param` (session compute.rs), after setting the value, run
  validation and return warnings alongside the success response
- Warnings don't block the set — they inform the user
- MCP response includes a `warnings: []` array

**Option B: Validation in properties panel**
- The existing `validate_toolpath` / `validate_toolpath_config` in
  `ui/properties/operations/mod.rs` already returns `Vec<String>` of errors
- Extend this with a `warn_toolpath` function that returns non-blocking warnings
- Show warnings as yellow text below the parameter slider

**Option C: Both** — validate at set time for MCP, validate in panel for GUI.

### How to compute recommended values

The auto-calculation already happens in the feeds system. The simplest approach:
1. When a user overrides a parameter, compute what the auto value would have been
2. Compare the user value against the auto value
3. If the ratio exceeds a threshold (e.g. 2x), emit a warning

The auto values are available via the `FeedsAutoMode` system — when a field is
set to `auto: true`, the system computes the value. We can call the same
computation even when `auto: false` to get the recommended baseline for comparison.

### Key files

| File | What it contains |
|------|-----------------|
| `crates/rs_cam_core/src/compute/operation_configs.rs` | All operation config structs |
| `crates/rs_cam_core/src/compute/feeds.rs` or similar | Feeds/speeds auto-calculation |
| `crates/rs_cam_core/src/machine.rs` | MachineProfile with rigidity factors |
| `crates/rs_cam_core/src/material.rs` | Material hardness and Kc values |
| `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` | `validate_toolpath`, UI validation |
| `crates/rs_cam_core/src/session/compute.rs` | `set_toolpath_param` |

### Priority

Medium-High — prevents users (and AI agents) from setting dangerous parameters
that produce broken toolpaths, tool breakage, or excessive cycle times. The AI
agent fuzz test showed how easy it is to set parameters that produce nonsensical
results (e.g. stepover=3mm > tool radius=1mm for a ball nose).
