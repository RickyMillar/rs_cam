# Review: Tool Geometry (5 Families)

## Summary

The tool geometry layer is well-architected around a fully dyn-compatible `MillingCutter` trait using the template-method pattern from OpenCAMLib. All 5 tool types implement correct edge_drop geometry with no major mathematical errors. Two minor numerical stability gaps exist in VBit and TaperedBall edge_drop (potential NaN from unsafeguarded sqrt), and test coverage is uneven — BullNose/VBit are well-tested while FlatEndmill has only 4 tests total.

## Findings

### Trait Design

- **Trait:** `MillingCutter: Send + Sync` at `tool/mod.rs:64-158`
- **Fully dyn-compatible:** No generic parameters, no associated types, all concrete return types
- **Send + Sync** bounds enable parallel rayon processing in `batch_drop_cutter`
- **Template-method pattern:** `drop_cutter()` (line 142) orchestrates facet -> vertices -> 3 edges; subclasses only implement profile functions + `edge_drop()`
- **Functions accept `&C: MillingCutter + ?Sized`** (e.g. `dropcutter.rs:11,52`) enabling both concrete and trait-object references
- **No type-matching in operations:** All callers use the trait polymorphically. No downcasting or `if let` type checks found anywhere in the codebase.
- **Collision envelope is separate:** `collision.rs:17-100` models holder/shank as `ToolAssembly` struct, not integrated with the trait. Detection creates a temporary `FlatEndmill` at holder radius (line 239).

**Methods exposed:**

| Method | Default | Purpose |
|--------|---------|---------|
| `diameter()` | No | Tool diameter (mm) |
| `radius()` | Yes (d/2) | Convenience |
| `length()` | No | Cutting flute length |
| `height_at_radius(r)` | No | Profile height at radius r |
| `width_at_height(h)` | No | Profile radius at height h |
| `center_height()` | No | Height of cutter center above tip |
| `normal_length()` | No | CC->CL offset along surface normal |
| `xy_normal_length()` | No | CC->CL offset in XY projected normal |
| `vertex_drop()` | Yes | Universal vertex contact formula |
| `facet_drop()` | Yes | Universal facet contact (overridden by VBit, TaperedBall) |
| `edge_drop()` | No | Per-tool edge contact geometry |
| `drop_cutter()` | Yes | Template: facet + vertices + edges |

### Per-Tool Review

#### Flat Endmill (`tool/flat.rs:26-105`)
- **Profile:** height(r) = 0 for r <= R
- **edge_drop:** Circle-line intersection in XY. Formula: `s = sqrt(R^2 - d^2) / edge_len_xy`
- **Correctness:** Sound. Proper degenerate edge handling (line 67).
- **Note:** Lines 87-92 contain dead code (abandoned approach) — computes but discards candidates. Not harmful but confusing.

#### Ball Endmill (`tool/ball.rs:26-159`)
- **Profile:** height(r) = R - sqrt(R^2 - r^2)
- **edge_drop:** Sphere-line tangency with slope matching. Cross-section circle + sin_a/cos_a normal matching.
- **Correctness:** Sound. Proper hemisphere validation (`sin_a >= -1e-10` at line 154).
- **Numerical stability:** Good — clamps `r.min(big_r)` before sqrt (line 39), checks edge_len_xy < 1e-20 (line 88).

#### Bull Nose Endmill (`tool/bullnose.rs:50-243`)
- **Profile:** Flat (r <= r1) + torus (r1 < r <= R), where r1 = R - corner_radius
- **edge_drop:** Two-region approach — flat region uses circle-line (like FlatEndmill), torus region uses simplified circular tube cross-section
- **Correctness:** Sound. Uses `.max(0.0)` before sqrt for numerical safety (lines 68, 82, 210-211).
- **Note:** This is a pragmatic simplification of the full offset-ellipse + Brent solver approach from OpenCAMLib. Acknowledged in research notes.

#### V-Bit Endmill (`tool/vbit.rs:55-246`)
- **Profile:** height(r) = r / tan(half_angle)
- **edge_drop:** Hyperbola intersection with two contact cases (conical surface vs. rim contact)
- **Correctness:** Sound mathematical formulation.
- **facet_drop override:** Two modes — tip contact (horizontal surfaces) and conical surface contact.
- **Issue:** Potential NaN at line 232 (see Issues below).

#### Tapered Ball Endmill (`tool/tapered_ball.rs:97-340`)
- **Profile:** Ball (r <= r_contact) + Cone (r > r_contact), composite geometry
- **edge_drop:** Delegates to ball region (sphere-line tangency) or cone region (hyperbola, like VBit)
- **facet_drop override:** Three modes — ball contact, cone contact, tip fallback
- **Correctness:** Sound. Proper boundary validation between ball/cone regions.
- **Issue:** Same potential NaN as VBit at line 322 (see Issues below).

### Algorithm Sources (cross-referenced with CREDITS.md)

All 5 tool types trace to OpenCAMLib (github.com/aewallin/opencamlib):
- Flat: `CylCutter::singleEdgeDropCanonical()`
- Ball: `BallCutter::singleEdgeDropCanonical()`
- Bull: `BullCutter::singleEdgeDropCanonical()` (simplified — no Brent solver)
- VBit: `ConeCutter::singleEdgeDropCanonical()`
- TaperedBall: Composite of ball + cone sub-cutter approach

Detailed research docs: `research/03_tool_geometry.md`, `research/raw_opencamlib_math.md`

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | **Potential NaN in VBit edge_drop**: `ccu_sq.sqrt()` without `.max(0.0)` guard. If rounding pushes `ccu_sq` slightly negative, produces NaN. | `tool/vbit.rs:231-232` |
| 2 | Med | **Potential NaN in TaperedBall edge_drop**: Same unsafeguarded sqrt as VBit in cone-region edge_drop. | `tool/tapered_ball.rs:321-322` |
| 3 | Low | **Dead code in Flat edge_drop**: Lines 87-92 compute and discard candidates before the correct computation on line 96. Not a bug but misleading. | `tool/flat.rs:87-92` |
| 4 | Low | **Bull nose uses simplified edge_drop**: Circular tube cross-section instead of full offset-ellipse + Brent solver. Adequate for wood routing but less accurate for hard metals with small corner radii. | `tool/bullnose.rs:142-207` |

**Recommended fix for issues 1 & 2:**
```rust
let ccu = ccu_sq.max(0.0).sqrt();  // Guard against negative rounding
```

## Test Gaps

### Coverage Summary

| Tool Type | Test Count | edge_drop Tests | Quality |
|-----------|-----------|----------------|---------|
| FlatEndmill | 4 | 1 | Basic |
| BallEndmill | 7 | 1 | Moderate |
| BullNoseEndmill | 20 | 4 | Comprehensive |
| VBitEndmill | 16 | 3 | Comprehensive |
| TaperedBallEndmill | 18 | 2 | Moderate |
| MillingCutter trait | 1 | 0 | Minimal |
| **Total** | **66** | **11** | |

Plus 2 integration tests in `tests/end_to_end.rs` using BallEndmill.

### Specific Gaps

1. **FlatEndmill severely undertested** — Only 4 tests total, 1 edge_drop test. Missing: vertical edge, sloped edge, edge tangent to tool, out-of-range edge.
2. **No vertical edge tests for any tool** — Code handles vertical edges with `edge_len_xy < 1e-20` checks (flat.rs:67, ball.rs:88, etc.) but no tests verify this path.
3. **No zero-area triangle tests** — Collinear vertices producing degenerate triangles untested.
4. **TaperedBall missing cone-region edge tests** — Only ball-region edge_drop tested; cone-region edge_drop is unvalidated.
5. **Loose assertions in some edge_drop tests** — Several tests only check `cl.z > f64::NEG_INFINITY` without validating actual Z values (bullnose.rs:462-464, vbit.rs:432-435, tapered_ball.rs:532-535).
6. **No multiple-contact-point tests** — Ball, bull nose, and VBit support multiple solutions (both `+sign` and `-sign`), but tests only verify that *some* contact is found.
7. **No exact-on-boundary tests** — Edge passing through CL axis (distance=0), edge tangent to tool radius.

## Suggestions

1. **Guard sqrt in VBit/TaperedBall** — Add `.max(0.0)` before `.sqrt()` on `ccu_sq` to prevent NaN from rounding errors.
2. **Add vertical edge tests** — All 5 tools have the guard but no test exercises it.
3. **Expand FlatEndmill tests** — Parity with BullNose/VBit coverage level.
4. **Add TaperedBall cone-region edge tests** — Currently a blind spot.
5. **Tighten loose assertions** — Replace `> NEG_INFINITY` checks with actual expected Z values (even approximate).
6. **Remove dead code in flat.rs:87-92** — Confusing artifact that serves no purpose.
7. **Consider property-based testing** — Invariants like "edge_drop Z <= vertex_drop Z for same triangle" could catch subtle geometry bugs across all tool types.
