# Defaults Analysis

Analysis of default values, their sensibility for wood routing, and how tool/stock changes should propagate.

---

## Default Value Inventory

### Feed rates (mm/min)

| Operation | Feed Rate | Plunge Rate | Sensible? | Notes |
|-----------|-----------|-------------|-----------|-------|
| Face | 1500 | 500 | OK | Aggressive for wood, reasonable for surfacing |
| Pocket | 1000 | 500 | OK | Conservative, good starting point |
| Profile | 1000 | 500 | OK | Match pocket |
| Adaptive | 1500 | 500 | OK | Higher because engagement is controlled |
| VCarve | 800 | 400 | OK | Conservative for V-bits |
| Rest | 1000 | 500 | OK | Match pocket |
| Inlay | 800 | 400 | OK | Match VCarve (uses V-bit) |
| Zigzag | 1000 | 500 | OK | Match pocket |
| Trace | 800 | 400 | OK | Conservative for engraving |
| Drill | 300 | — | OK | Slow for plunging (drilling is all plunge) |
| Chamfer | 800 | 400 | OK | Match VCarve |
| DropCutter | 1000 | 500 | OK | Conservative 3D finish |
| Adaptive3D | 1500 | 500 | OK | Match 2D adaptive |
| Waterline | 1000 | 500 | OK | Standard finish |
| Pencil | 800 | 400 | OK | Light finishing pass |
| Scallop | 1000 | 500 | OK | Standard finish |
| SteepShallow | 1000 | 500 | OK | Standard finish |
| RampFinish | 1000 | 500 | OK | Standard finish |
| SpiralFinish | 1000 | 500 | OK | Standard finish |
| RadialFinish | 1000 | 500 | OK | Standard finish |
| HorizontalFinish | 1000 | 500 | OK | Standard finish |
| ProjectCurve | 800 | 400 | OK | Light engraving pass |

**Assessment**: Feed/plunge defaults are reasonable starting points. The auto-feeds system overrides these when enabled (default), so these are fallbacks when auto is disabled.

### Stepover defaults (mm)

| Operation | Default | Sensible? | Notes |
|-----------|---------|-----------|-------|
| Face | 5.0 | QUESTIONABLE | Too large for small tools (6mm end mill → 83% WOC); OK for 1/2" surfacing bit |
| Pocket | 2.0 | OK | ~40% of typical 1/4" tool (6.35mm). Good for clearing |
| Adaptive | 2.0 | OK | Standard for controlled-engagement clearing |
| VCarve | 0.5 | OK | Fine for V-carving scan lines |
| Rest | 1.0 | OK | Fine scan line spacing |
| Inlay | 1.0 | OK | Fine for inlay V-carving |
| Zigzag | 2.0 | OK | Match pocket |
| DropCutter | 1.0 | OK | Standard for 3D finishing |
| Adaptive3D | 2.0 | OK | Standard for 3D roughing |
| SteepShallow | 1.0 | OK | Standard finish |
| SpiralFinish | 1.0 | OK | Standard finish |
| HorizontalFinish | 1.0 | OK | Standard finish |

**Key concern**: Stepover defaults are ABSOLUTE (mm), not relative to tool diameter. A 2mm stepover is fine for a 6mm tool (33%) but aggressive for a 3mm tool (67%) and wasteful for a 25mm tool (8%). The auto-feeds system computes appropriate stepover when enabled.

### Depth defaults (mm)

| Operation | Depth | Depth/Pass | Sensible? | Notes |
|-----------|-------|------------|-----------|-------|
| Face | 0.0 / 1.0 | 1.0 | OK | 0 depth = skim surface; 1mm/pass is conservative |
| Pocket | 3.0 / 1.5 | 1.5 | OK | Reasonable pocket depth for wood |
| Profile | 6.0 / 2.0 | 2.0 | OK | Through-cut for 1/4" material |
| Adaptive | 6.0 / 2.0 | 2.0 | OK | Match profile |
| Zigzag | 3.0 / 1.5 | 1.5 | OK | Match pocket |
| Rest | 6.0 / 2.0 | 2.0 | OK | Match profile depth |
| Adaptive3D | — / 3.0 | 3.0 | OK | Larger stepdown for 3D roughing |
| Drill | 10.0 / 3.0 (peck) | — | OK | Standard through-hole |
| Trace | 1.0 / 0.5 | 0.5 | OK | Shallow engraving |
| ProjectCurve | 1.0 / — | — | OK | Surface engraving |

**Assessment**: Depth defaults are reasonable for common wood routing. They don't auto-adjust for tool or stock changes.

---

## What Should Change When Tool Changes

When the user selects a different tool, these parameters SHOULD be affected:

### Currently auto-updated (when feeds_auto toggles are ON)

| Parameter | How it changes | Source |
|-----------|---------------|--------|
| `feed_rate` | Recalculated from chipload × flute_count × RPM | Feeds calculator |
| `plunge_rate` | Typically 50% of feed_rate | Feeds calculator |
| `stepover` | Typically 30-50% of tool diameter for roughing, scallop-based for finishing | Feeds calculator |
| `depth_per_pass` | Based on tool diameter, material, flute length | Feeds calculator |
| `spindle_speed` | From SFM / tool diameter | Feeds calculator |

### NOT auto-updated (but arguably should be)

| Parameter | What should happen | Current behavior | Risk |
|-----------|-------------------|-----------------|------|
| `safe_z` / heights | No change needed (stock-relative) | Auto mode resolves from context | OK |
| `tolerance` | Could scale with tool diameter | Static default | Low risk |
| `min_cutting_radius` | Meaningless if > tool_radius | Static default (0.0) | Low risk |
| `stock_offset` (Face) | Could scale with tool diameter | Static 5.0mm | Minor — too small offset for big tool |
| `lead_radius` (dressup) | Should be ~tool_radius | Static 2.0mm | Could be too small for large tools |
| `helix_radius` (dressup) | Should be ~tool_radius | Static 2.0mm | Could be too small for large tools |
| `link_max_distance` (dressup) | Could scale with tool diameter | Static 10.0mm | Minor |
| `tab_width` (Profile) | Could scale with tool diameter | Static 6.0mm | Minor |

### What should change when tool TYPE changes

| Change | Affected parameters |
|--------|-------------------|
| EndMill → V-Bit | Operations requiring V-bit (VCarve, Chamfer, Inlay) become valid; `half_angle` derived from tool |
| EndMill → BallNose | Scallop height calculation changes (wider effective stepover for same scallop); pencil/drop-cutter surface quality improves |
| Flat → Bull Nose | Corner radius affects effective cutting diameter at shallow depths |
| Any → Tapered Ball | Taper angle affects drop-cutter geometry and reach |

---

## What Should Change When Stock Changes

### Stock dimensions change

| Parameter | Expected behavior | Current behavior | Status |
|-----------|------------------|-----------------|--------|
| Heights (all 5) | Auto-recompute from new stock bounds | Yes — `HeightMode::Auto` resolves fresh | OK |
| `stock_top_z` (Adaptive3D) | Should match new stock top | Static default 30.0mm | **BUG RISK**: stale if stock changes |
| `min_z` (DropCutter) | Should match or exceed new stock bottom | Static default -50.0mm | Low risk (conservative default) |
| `start_z` / `final_z` (Waterline) | Should bracket new model/stock Z range | Static 0.0 / -25.0 | **BUG RISK**: stale if stock changes |
| Clip to Stock boundary | Uses current stock bounds | Dynamic (recomputed at generation time) | OK |

### Stock material changes

| Parameter | Expected behavior | Current behavior | Status |
|-----------|------------------|-----------------|--------|
| Feed rate | Recalculate from new material chipload/SFM | Only if feeds_auto is ON and Feeds tab is rendered | OK (with caveat) |
| Depth per pass | Recalculate from new material hardness | Only if feeds_auto is ON | OK (with caveat) |
| Stepover | May need adjustment for harder materials | Only if feeds_auto is ON | OK (with caveat) |

**Caveat**: Auto-feeds only recalculate when the Feeds tab is rendered. If the user changes material but never opens the Feeds tab, the old values persist.

---

## Sensible Default Recommendations

### Defaults that should be tool-relative but aren't

| Parameter | Current default | Better default | Formula |
|-----------|----------------|---------------|---------|
| Stepover (all) | Fixed mm | % of tool diameter | Roughing: 40% of Ø; Finishing: scallop-derived |
| `stock_offset` (Face) | 5.0 mm | tool_diameter × 0.5 | Ensure full coverage at edges |
| `lead_radius` (dressup) | 2.0 mm | tool_radius × 0.5 | Scale with tool |
| `helix_radius` (dressup) | 2.0 mm | tool_radius × 1.5 | Must be > tool_radius |
| `link_max_distance` (dressup) | 10.0 mm | tool_diameter × 3 | Scale with tool |

### Defaults that should be stock-relative but aren't

| Parameter | Current default | Better default | Formula |
|-----------|----------------|---------------|---------|
| `stock_top_z` (Adaptive3D) | 30.0 mm | stock.top_z | Match actual stock |
| `start_z` (Waterline) | 0.0 mm | model.top_z or stock.top_z | Match geometry |
| `final_z` (Waterline) | -25.0 mm | model.bottom_z or stock.bottom_z | Match geometry |
| `min_z` (DropCutter) | -50.0 mm | stock.bottom_z - 1mm | Match stock |

### Defaults that are arguably wrong

| Parameter | Current | Issue | Suggested |
|-----------|---------|-------|-----------|
| `scallop_height` (Scallop) | 0.1 mm | Very fine for wood routing — will be slow | 0.2 mm for wood |
| `slope_from` (RampFinish) | 30.0 deg | Excludes shallow walls; 0 would include everything | 0.0 deg |
| `overlap_distance` (SteepShallow) | 1.0 mm | Relatively small; 2× stepover is common | 2.0 mm (2× default stepover) |
| `wall_clearance` (SteepShallow) | 0.5 mm | Small; could leave witness marks | 1.0 mm |
| `threshold_angle` (SteepShallow) | 45.0 deg | Standard, but core default was 40.0 — inconsistency | Align to one value |

---

## Auto-Feeds Propagation Gaps

The auto-feeds system is the PRIMARY mechanism for tool-aware defaults, but it has gaps:

1. **Trigger**: Only runs when Feeds tab is rendered in the GUI. Changing tool/material doesn't immediately propagate.
2. **Scope**: Only controls feed_rate, plunge_rate, stepover, depth_per_pass, spindle_speed. Other tool-sensitive params (lead radius, helix radius, etc.) are never auto-updated.
3. **Workholding rigidity**: Hardcoded to `Medium` in the GUI — not user-configurable despite being in the feeds system.
4. **Operation mapping**: Some operations may map to suboptimal family/role combos (e.g., Drill feeds aren't auto-calculated via the feeds system at all).
