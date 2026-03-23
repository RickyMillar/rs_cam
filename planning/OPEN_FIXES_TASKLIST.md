# Open / Overstated Fixes Task List

## Completed in this pass

- [x] `A2`: remove the remaining degrees-to-radians mismatches in 2D raster execution and semantic reconstruction
- [x] `I1`: thread real tool numbers through Viz and CLI G-code export
- [x] `I2`: thread typed coolant settings through Viz project state and CLI job execution into G-code export
- [x] `C11`: cancel the analysis lane when resetting simulation state
- [x] `G4`: fit the camera to imported 2D SVG/DXF model bounds instead of mesh-only bounds
- [x] `G9` / `G10`: move geometry/type checks into the shared validator and enforce earlier same-setup rest-source ordering
- [x] Add regression coverage for the angle fixes, export wiring, controller reset, and project persistence

## Remaining high-value items

- [ ] `A10`: split inlay female and male output into separate GUI-visible toolpath artifacts instead of one combined `Toolpath`
- [ ] `C12`: add a bounded queue or drop/replace policy for toolpath compute submissions
- [ ] `C16`: remove the remaining `ModelId(0)` fallback structurally, likely by making model references optional where geometry is not required
- [ ] `D1`: implement actual thick-line rendering instead of storing width config only
- [ ] `B5`: compute deviation data for simulation or hide deviation coloring until it exists
- [ ] `G9` / `G10`: finish central validation so geometry presence and rest-operation ordering are enforced in one place

## Notes

- The `ModelId(0)` problem is now surfaced much earlier by shared validation, and geometry-requiring toolpaths are blocked on creation without a model. The remaining work is the structural cleanup of the fallback itself.
- Coolant/tool-number export is now wired end-to-end through typed state, but the GUI still lacks a dedicated editor for those fields.
