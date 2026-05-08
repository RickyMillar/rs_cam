# Step 3 prep — `OperationConfig::optimization_surface()` arms

Pre-work for G16 Step 3. Maps every `OperationConfig` variant to its
`OptimizationSurface` arm + `&'static [AxisBinding]`. Step 3 becomes
transcription of this table to Rust.

Authored 2026-05-08 in parallel with reviewer's Step 1 (policy.rs
extract). Does not touch `optimize.rs` — pure design work.

## Variant → axes mapping

23 variants total. 21 Optimizable, 2 NotOptimizable.

| Variant            | Status          | Axes (`SearchAxis`)                                | Notes |
|--------------------|-----------------|----------------------------------------------------|-------|
| `Face`             | Optimizable     | FeedRate, SpindleRpm, DepthPerPass, Stepover       | 2.5D clearing |
| `Pocket`           | Optimizable     | FeedRate, SpindleRpm, DepthPerPass, Stepover       | 2.5D clearing |
| `Profile`          | Optimizable     | FeedRate, SpindleRpm, DepthPerPass                 | Contour-follow; no stepover semantics |
| `Adaptive`         | Optimizable     | FeedRate, SpindleRpm, DepthPerPass, Stepover       | 2D HSM adaptive |
| `VCarve`           | Optimizable     | FeedRate, SpindleRpm, Stepover                     | V-bit projection (no DOC; G4 future) |
| `Rest`             | Optimizable     | FeedRate, SpindleRpm, DepthPerPass, Stepover       | Rest machining |
| `Inlay`            | Optimizable     | FeedRate, SpindleRpm, Stepover                     | (no DOC; G4 future) |
| `Zigzag`           | Optimizable     | FeedRate, SpindleRpm, DepthPerPass, Stepover       | G1 added DOC |
| `Trace`            | Optimizable     | FeedRate, SpindleRpm, DepthPerPass                 | G3 added DOC; no stepover |
| `Drill`            | **NotOptimizable** | —                                               | reason: `SteadyStateSamplesNotPresent` |
| `Chamfer`          | Optimizable     | FeedRate, SpindleRpm                               | V-bit; G4 may add DOC/stepover later |
| `DropCutter`       | Optimizable     | FeedRate, SpindleRpm, Stepover                     | 3D scan finish |
| `Adaptive3d`       | Optimizable     | FeedRate, SpindleRpm, DepthPerPass, Stepover       | 3D HSM clearing |
| `Waterline`        | Optimizable     | FeedRate, SpindleRpm, DepthPerPass                 | G3 added DOC; `z_step` is the DOC field |
| `Pencil`           | Optimizable     | FeedRate, SpindleRpm, Stepover (conditional)       | G3: stepover only Some when `num_offset_passes > 1` |
| `Scallop`          | Optimizable     | FeedRate, SpindleRpm, ScallopHeight                | G2 added scallop_height axis |
| `SteepShallow`     | Optimizable     | FeedRate, SpindleRpm, Stepover                     | |
| `RampFinish`       | Optimizable     | FeedRate, SpindleRpm, DepthPerPass                 | G3 added DOC; `max_stepdown` is the DOC field |
| `SpiralFinish`     | Optimizable     | FeedRate, SpindleRpm, Stepover                     | |
| `RadialFinish`     | Optimizable     | FeedRate, SpindleRpm                               | G3a (future): `AngularStep` axis to add |
| `HorizontalFinish` | Optimizable     | FeedRate, SpindleRpm, Stepover                     | |
| `ProjectCurve`     | Optimizable     | FeedRate, SpindleRpm                               | No geometry knobs; feed/RPM only |
| `AlignmentPinDrill`| **NotOptimizable** | —                                               | reason: `SteadyStateSamplesNotPresent` |

## Per-axis `AxisBinding` constants

These get declared once at module scope and referenced from each
op's `&'static [AxisBinding]`. Each carries the axis, its semantics,
and the field name on the underlying config struct (for debugging /
log output).

```rust
const BIND_FEED: AxisBinding = AxisBinding {
    axis: SearchAxis::FeedRate,
    field_name: "feed_rate",
    unit: AxisUnit::MmPerMin,
    semantics: AxisSemantics::LoadDriving {
        affects_chipload: true,
        affects_force: true,
    },
};

const BIND_RPM: AxisBinding = AxisBinding {
    axis: SearchAxis::SpindleRpm,
    field_name: "spindle_rpm",
    unit: AxisUnit::Rpm,
    semantics: AxisSemantics::LoadDriving {
        affects_chipload: true,
        affects_force: true,
    },
};

const BIND_DOC: AxisBinding = AxisBinding {
    axis: SearchAxis::DepthPerPass,
    field_name: "depth_per_pass",
    unit: AxisUnit::Mm,
    semantics: AxisSemantics::LoadDriving {
        affects_chipload: false,
        affects_force: true,
    },
};

const BIND_STEPOVER: AxisBinding = AxisBinding {
    axis: SearchAxis::Stepover,
    field_name: "stepover",
    unit: AxisUnit::Mm,
    semantics: AxisSemantics::LoadDriving {
        affects_chipload: true,   // via radial chip thinning
        affects_force: true,
    },
};

const BIND_SCALLOP_HEIGHT: AxisBinding = AxisBinding {
    axis: SearchAxis::ScallopHeight,
    field_name: "scallop_height",
    unit: AxisUnit::Mm,
    semantics: AxisSemantics::QualityTarget,
};

// Future, when G3a closes:
const BIND_ANGULAR_STEP: AxisBinding = AxisBinding {
    axis: SearchAxis::AngularStep,
    field_name: "angular_step",
    unit: AxisUnit::Deg,
    semantics: AxisSemantics::QualityTarget,
};
```

## Per-variant `&'static [AxisBinding]`

Five distinct shapes cover all 21 optimizable variants:

```rust
const FEED_RPM_ONLY: &[AxisBinding] = &[BIND_FEED, BIND_RPM];

const FEED_RPM_DOC: &[AxisBinding] = &[BIND_FEED, BIND_RPM, BIND_DOC];

const FEED_RPM_STEPOVER: &[AxisBinding] = &[BIND_FEED, BIND_RPM, BIND_STEPOVER];

const FEED_RPM_DOC_STEPOVER: &[AxisBinding] =
    &[BIND_FEED, BIND_RPM, BIND_DOC, BIND_STEPOVER];

const FEED_RPM_SCALLOP: &[AxisBinding] = &[BIND_FEED, BIND_RPM, BIND_SCALLOP_HEIGHT];
```

Variant → bindings:

| Variant | Bindings |
|---|---|
| Face, Pocket, Adaptive, Rest, Zigzag, Adaptive3d | `FEED_RPM_DOC_STEPOVER` |
| Profile, Trace, Waterline, RampFinish | `FEED_RPM_DOC` |
| VCarve, Inlay, DropCutter, Pencil, SteepShallow, SpiralFinish, HorizontalFinish | `FEED_RPM_STEPOVER` |
| Scallop | `FEED_RPM_SCALLOP` |
| Chamfer, ProjectCurve, RadialFinish | `FEED_RPM_ONLY` |
| Drill, AlignmentPinDrill | (NotOptimizable) |

## The `optimization_surface()` arm

```rust
impl OperationConfig {
    pub fn optimization_surface(&self) -> OptimizationSurface<'_> {
        match self {
            OperationConfig::Face(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC_STEPOVER,
                op_type: OperationType::Face,
            }),
            OperationConfig::Pocket(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC_STEPOVER,
                op_type: OperationType::Pocket,
            }),
            OperationConfig::Profile(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC,
                op_type: OperationType::Profile,
            }),
            OperationConfig::Adaptive(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC_STEPOVER,
                op_type: OperationType::Adaptive,
            }),
            OperationConfig::VCarve(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_STEPOVER,
                op_type: OperationType::VCarve,
            }),
            OperationConfig::Rest(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC_STEPOVER,
                op_type: OperationType::Rest,
            }),
            OperationConfig::Inlay(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_STEPOVER,
                op_type: OperationType::Inlay,
            }),
            OperationConfig::Zigzag(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC_STEPOVER,
                op_type: OperationType::Zigzag,
            }),
            OperationConfig::Trace(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC,
                op_type: OperationType::Trace,
            }),
            OperationConfig::Drill(_) => OptimizationSurface::NotOptimizable {
                reason: RefuseReason::SteadyStateSamplesNotPresent,
            },
            OperationConfig::Chamfer(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_ONLY,
                op_type: OperationType::Chamfer,
            }),
            OperationConfig::DropCutter(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_STEPOVER,
                op_type: OperationType::DropCutter,
            }),
            OperationConfig::Adaptive3d(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC_STEPOVER,
                op_type: OperationType::Adaptive3d,
            }),
            OperationConfig::Waterline(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC,
                op_type: OperationType::Waterline,
            }),
            OperationConfig::Pencil(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_STEPOVER,
                op_type: OperationType::Pencil,
            }),
            OperationConfig::Scallop(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_SCALLOP,
                op_type: OperationType::Scallop,
            }),
            OperationConfig::SteepShallow(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_STEPOVER,
                op_type: OperationType::SteepShallow,
            }),
            OperationConfig::RampFinish(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_DOC,
                op_type: OperationType::RampFinish,
            }),
            OperationConfig::SpiralFinish(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_STEPOVER,
                op_type: OperationType::SpiralFinish,
            }),
            OperationConfig::RadialFinish(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_ONLY,
                op_type: OperationType::RadialFinish,
            }),
            OperationConfig::HorizontalFinish(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_STEPOVER,
                op_type: OperationType::HorizontalFinish,
            }),
            OperationConfig::ProjectCurve(_) => OptimizationSurface::Optimizable(AxisView {
                op: self, bindings: FEED_RPM_ONLY,
                op_type: OperationType::ProjectCurve,
            }),
            OperationConfig::AlignmentPinDrill(_) => OptimizationSurface::NotOptimizable {
                reason: RefuseReason::SteadyStateSamplesNotPresent,
            },
            // No wildcard arm. Adding a new variant forces explicit
            // classification at compile time.
        }
    }
}
```

## `axis_value` / `set_axis` resolution

Both delegate to existing `OperationParams` accessors. **`SpindleRpm`
is the special case** — `op.spindle_rpm()` returns `Option<u32>` where
None means "use project default", not "axis absent". The `AxisView`
must consult `AxisContext::project_default_rpm` for that case.

```rust
impl<'op> AxisView<'op> {
    pub fn axis_value(&self, axis: SearchAxis, ctx: &AxisContext<'_>) -> Option<f64> {
        match axis {
            SearchAxis::FeedRate => Some(self.op.feed_rate()),
            SearchAxis::SpindleRpm => Some(
                self.op.spindle_rpm()
                    .map(f64::from)
                    .unwrap_or(f64::from(ctx.project_default_rpm))
            ),
            SearchAxis::DepthPerPass => self.op.depth_per_pass(),
            SearchAxis::Stepover => self.op.stepover(),
            SearchAxis::ScallopHeight => self.op.scallop_height(),
            SearchAxis::AngularStep => None,    // G3a future
            SearchAxis::HelixPitch | SearchAxis::RampAngle => None,  // future
        }
    }
}
```

`set_axis` lives in the patch-application path; signature:

```rust
pub fn apply_axis_patch_to_op(
    op: &mut OperationConfig,
    axis: SearchAxis,
    value: f64,
) -> Result<(), AxisError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(AxisError::InvalidValue { axis, value });
    }
    match axis {
        SearchAxis::FeedRate => { op.set_feed_rate(value); Ok(()) }
        SearchAxis::SpindleRpm => { op.set_spindle_rpm(Some(value as u32)); Ok(()) }
        SearchAxis::DepthPerPass => { op.set_depth_per_pass(value); Ok(()) }
        SearchAxis::Stepover => { op.set_stepover(value); Ok(()) }
        SearchAxis::ScallopHeight => { op.set_scallop_height(value); Ok(()) }
        SearchAxis::AngularStep
        | SearchAxis::HelixPitch
        | SearchAxis::RampAngle => Err(AxisError::NotImplemented { axis }),
    }
}
```

The default no-op `set_*` impls on `OperationParams` for axes the op
doesn't carry mean `apply_axis_patch_to_op` is a silent no-op for
mismatched axis × op pairs. **Acceptable** because the optimizer only
emits patches for axes declared in the op's `bindings`, so a mismatch
means upstream policy violation, not patch application bug.

A defensive belt-and-braces: validate `axis ∈ view.bindings` before
calling. Cheap.

## Coverage test required

```rust
#[test]
fn every_op_has_explicit_optimization_surface() {
    for &op_type in OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        let surface = op.optimization_surface();
        match surface {
            OptimizationSurface::Optimizable(view) => {
                assert!(!view.bindings.is_empty(),
                    "{op_type:?} declared Optimizable with no bindings");
                assert_eq!(view.op_type, op_type);
            }
            OptimizationSurface::NotOptimizable { reason } => {
                assert!(matches!(op_type,
                    OperationType::Drill | OperationType::AlignmentPinDrill),
                    "{op_type:?} unexpectedly NotOptimizable: {reason:?}");
            }
        }
    }
}

#[test]
fn axis_value_resolves_for_every_declared_axis() {
    let ctx = test_axis_context();
    for &op_type in OperationType::ALL {
        let op = OperationConfig::new_default(op_type);
        if let OptimizationSurface::Optimizable(view) = op.optimization_surface() {
            for binding in view.bindings {
                let v = view.axis_value(binding.axis, &ctx);
                assert!(v.is_some() || binding.axis == SearchAxis::Stepover,
                    "{op_type:?}.{:?} returned None for declared binding", binding.axis);
                // Stepover may legitimately be None (Pencil conditional);
                // every other declared axis must resolve.
            }
        }
    }
}
```

## Open: Pencil conditional stepover

`PencilConfig::stepover()` returns `Some` only when `num_offset_passes > 1`.
The binding is declared (it's `FEED_RPM_STEPOVER`), but `axis_value`
returns None at runtime when the conditional fails. Orchestrator must
filter declared-but-runtime-absent axes:

```rust
let active_axes: Vec<_> = view.bindings.iter()
    .filter(|b| view.axis_value(b.axis, ctx).is_some())
    .collect();
```

Documented; the test above tolerates this case.

## Where this content lands at Step 3

- `tool_load/optimize/axes.rs` — `SearchAxis`, `AxisUnit`,
  `AxisSemantics`, `AxisBinding`, `AxisView`, `AxisContext`, the
  `BIND_*` constants and `FEED_RPM_*` arrays, the `axis_value` impl.
- `compute/catalog.rs` — `OptimizationSurface` enum + the
  `optimization_surface()` method on `OperationConfig`.
- `tool_load/optimize/patches.rs` — `AxisPatch`, `apply_axis_patch_to_op`.
- `tool_load/optimize/tests.rs` (or inline) — the two coverage tests.

Total Rust: ~250 LOC. Mostly mechanical given this prep doc.
