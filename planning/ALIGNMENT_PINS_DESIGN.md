# Stock-Level Alignment Pins: Design Document

## Why

Alignment pins are physical dowels inserted into holes drilled through the
stock and into the spoilboard. They persist across all setups — when the
operator flips the stock, the same pin holes register on the same dowels.
Currently `alignment_pins` lives on `Setup`, which means each setup has its
own independent list of pins. This is wrong: pins are a property of the
stock, not a single setup.

Moving pins to the stock definition also unlocks:
- Automatic pin-hole drilling as the first operation
- Symmetry enforcement about the flip axis
- Pin instructions on every setup's sheet (not just the one that defined them)
- Stock carry-forward: the simulation can show pin holes drilled in Setup 1
  visible in Setup 2

## Industry research

### Vectric (VCarve Pro / Aspire) — closest to our target

- Two-sided machining is a **project-level mode** set at creation time.
- User specifies a **flip direction** (horizontal = left↔right, vertical =
  front↔back). The software auto-mirrors geometry between Side 1 / Side 2.
- Pins are user-placed circles, machined as drill/profile operations.
- **Center-of-stock origin** is recommended because it sits on the flip axis
  and doesn't move when flipped.

### Fusion 360

- No dedicated two-sided mode; uses general multi-setup WCS.
- Pin holes are regular CAD geometry — no special pin concept.
- "Stock from previous setup" imports the as-machined mesh.
- Requires manual WCS math for the flipped setup.

### Physical workflow (universal for wood CNC)

1. Machine Side 1 including pin holes (drilled through stock into spoilboard).
2. Remove stock, insert dowel pins into spoilboard holes.
3. Flip stock onto pins — pins register through the existing holes.
4. Re-zero Z only (pins handle X/Y).
5. Machine Side 2.

### Critical constraints

- Pin positions must be **symmetric about the flip axis** or the part shifts.
- Minimum 2 pins required (constrains translation + rotation).
- Pins should be in waste / margin area, not in the final part.
- Larger pins (6–8 mm) give better alignment than small ones.

## Data model changes

### Move pins from Setup to StockConfig

```rust
// In StockConfig (state/job.rs):
pub struct StockConfig {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
    pub auto_from_model: bool,
    pub padding: f64,
    pub material: Material,
    pub alignment_pins: Vec<AlignmentPin>,   // NEW — moved from Setup
    pub flip_axis: Option<FlipAxis>,         // NEW
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlipAxis {
    /// Flip left↔right (mirror about the X centerline, Y stays).
    Horizontal,
    /// Flip front↔back (mirror about the Y centerline, X stays).
    Vertical,
}
```

### AlignmentPin stays the same

```rust
pub struct AlignmentPin {
    pub x: f64,       // stock-relative X (mm)
    pub y: f64,       // stock-relative Y (mm)
    pub diameter: f64, // hole diameter (mm), default 6.0
}
```

Coordinates are stock-relative (origin at stock corner 0,0), NOT world
coordinates. This makes them independent of stock origin placement.

### Setup changes

- Remove `alignment_pins: Vec<AlignmentPin>` from `Setup`.
- Setups that need pins simply reference `job.stock.alignment_pins`.
- `XYDatum::AlignmentPins` remains valid — it means "use the stock's pins
  for this setup's XY registration."

## UI changes

### Stock panel (new "Alignment Pins" section)

Add a collapsible section to the stock properties panel:

```
▾ Alignment Pins
  Flip axis: [Horizontal ▾]     ← dropdown: None / Horizontal / Vertical
  Pin diameter: [6.0] mm

  Pin 1:  X [10.0]  Y [55.0]   [Mirror] [×]
  Pin 2:  X [100.0] Y [55.0]   [Mirror] [×]

  [+ Add Pin]   [Auto-place]
```

- **Flip axis** dropdown controls which axis pins must be symmetric about.
  When set, the UI shows a dashed centerline on the viewport and warns if
  pins are not symmetric.
- **Mirror button** per pin: creates the symmetric counterpart about the
  flip axis (or snaps the existing mirrored pin to the exact position).
- **Auto-place button**: places 2 pins at sensible default positions in the
  stock margin (e.g., 10mm from each edge, centered on the flip axis).
- **Pin diameter** is shared across all pins (physical dowels are one size).

### Setup panel changes

- Remove the per-setup pin list editor.
- When `xy_datum == AlignmentPins`, show a read-only reference:
  "Using N stock pins for registration."
- If no stock pins are defined and user selects AlignmentPins datum, prompt
  them to add pins on the stock panel.

### Viewport rendering

- Pins render as green circles (not crosses) at their XY positions,
  projected to stock top Z.
- When flip_axis is set, render a dashed centerline along that axis.
- Pins are rendered in ALL workspaces where stock is visible (Setup,
  Toolpaths, Simulation), not just the active setup.

### Setup sheet

- Pin table appears in the **Stock** section of the setup sheet (not
  per-setup).
- Each setup's instructions reference the stock pins:
  "Insert 6mm dowels into alignment pin holes. Flip stock [horizontally /
  vertically]. Part self-locates on pins."

## Pin-hole drilling operation (future phase)

When pins are defined, auto-generate a drilling toolpath:
- Added to Setup 1 as the first (or last pre-cutout) operation.
- Uses a user-selected drill bit or endmill.
- Plunge at each pin (x, y) to full stock depth + 2mm into spoilboard.
- The operation is flagged as "auto-generated" so it can be regenerated
  when pins change.

This is a separate phase because it requires toolpath generation changes
in rs_cam_core.

## Serialization

### TOML format change

Before (pins on setup):
```toml
[[setups]]
id = 0
alignment_pins = [{ x = 10.0, y = 55.0, diameter = 6.0 }]
```

After (pins on stock):
```toml
[job.stock]
x = 110.0
y = 110.0
z = 10.6
flip_axis = "horizontal"

[[job.stock.alignment_pins]]
x = 10.0
y = 55.0
diameter = 6.0

[[job.stock.alignment_pins]]
x = 100.0
y = 55.0
diameter = 6.0
```

### Migration

When loading a project with the old format (pins on setup), migrate:
- Collect all unique pins from all setups.
- Move them to `job.stock.alignment_pins`.
- Clear `setup.alignment_pins`.
- Log a warning: "Alignment pins migrated from setup to stock."

## Implementation phases

### Phase 1: Data model migration (backend)

- Move `alignment_pins` and add `flip_axis` to `StockConfig`.
- Remove `alignment_pins` from `Setup`.
- Update serialization with migration for old format.
- Update all references (rendering, picking, setup sheet, datum).
- Update tests.

### Phase 2: UI — stock panel pin editor

- Add the "Alignment Pins" section to the stock properties panel.
- Flip axis dropdown, pin list editor, add/remove buttons.
- Mirror button and symmetry validation.
- Auto-place button for sensible defaults.

### Phase 3: Viewport — pin rendering from stock

- Render pins from `job.stock.alignment_pins` instead of per-setup.
- Render in all workspaces where stock is visible.
- Render flip axis centerline when flip_axis is set.
- Update picking to reference stock pins.

### Phase 4: Pin-hole drilling operation (future)

- Auto-generate drilling toolpath from pin definitions.
- Integrate with toolpath ordering and simulation.

## Files to modify

| File | Phase | Changes |
|------|-------|---------|
| `crates/rs_cam_viz/src/state/job.rs` | 1 | Move pins + add FlipAxis to StockConfig, remove from Setup |
| `crates/rs_cam_viz/src/io/project.rs` | 1 | Serialization + migration |
| `crates/rs_cam_viz/src/io/setup_sheet.rs` | 1 | Pin table in stock section |
| `crates/rs_cam_viz/src/ui/properties/setup.rs` | 1,2 | Remove pin editor, add stock reference |
| `crates/rs_cam_viz/src/ui/properties/stock.rs` | 2 | New pin editor section |
| `crates/rs_cam_viz/src/app.rs` | 3 | Render pins from stock, update picking |
| `crates/rs_cam_viz/src/interaction/picking.rs` | 3 | Pick stock pins instead of setup pins |
| `crates/rs_cam_viz/src/ui/setup_panel.rs` | 1 | Update pin count chip to read from stock |
