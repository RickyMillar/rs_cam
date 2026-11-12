//! The input of the feeds calculator (feeds contract P29-7 commit 7a).
//!
//! [`FeedsInput`] has one constructor, [`FeedsInput::of_operation`]. The
//! facts of the calculator input come from the operation, and only from it:
//! the row key (family, role, kind), the stated depth, width and scallop,
//! the power cases and the facts of the body cut
//! (`operation.cuts(..).body_facts()`). The private zero-size field `door`
//! makes every struct literal and every struct update outside this module
//! fail (E0451), the lib tests of `feeds` included. So no code can build a
//! calculator input that states a cut fact that no op gives.
//!
//! A few named `with_*` methods change an input that is not a cut fact.
//! Each states in its doc why it exists. No `with_*` sets a width, a depth,
//! a power case, a role or a body fact: a test states those on the op.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::tool_config::ToolConfig;
use crate::limits::CutFacts;
use crate::machine::MachineProfile;
use crate::material::Material;

use super::{
    CutBody, OperationFamily, PassRole, SetupContext, SpindleStrategy, ToolGeometryHint, VendorLut,
    planned, series, sourced, vendor_lut,
};

/// The private door of [`FeedsInput`]: only this module can name it, so
/// only this module can build a `FeedsInput`.
#[derive(Debug, Clone, Copy)]
struct Door;

/// The input of the feeds calculator. Build it with
/// [`FeedsInput::of_operation`].
pub struct FeedsInput<'a> {
    pub(in crate::feeds) tool_diameter: f64,
    pub(in crate::feeds) flute_count: u32,
    pub(in crate::feeds) flute_length: f64,
    /// The shank in the collet (mm): Step 4c (a shank larger than the
    /// machine collet) is its only reader.
    pub(in crate::feeds) shank_diameter: Option<f64>,
    /// P22-0c-6b: the facts of the tool body that the feeds model reads
    /// (the cutting cap and the core), from the body that `build_cutter`
    /// builds ([`CutBody::of_tool`]).
    pub(in crate::feeds) cut_body: CutBody,
    pub(in crate::feeds) tool_geometry: ToolGeometryHint,
    /// The tool's cutter series (operator ruling 2026-10-31,
    /// [`series::ToolSeries`]): `None` is a generic (plain upcut) tool, and
    /// the vendor lookup reads the standard rows first. The constructor
    /// reads `ToolConfig::cut_direction`.
    pub(in crate::feeds) tool_series: Option<series::ToolSeries>,
    pub(in crate::feeds) material: &'a Material,
    pub(in crate::feeds) machine: &'a MachineProfile,
    /// The vendor row key: the op's feeds family. A row key, not a scope:
    /// Step 5 reads the thinning rule from [`Self::body_facts`].
    pub(in crate::feeds) operation: OperationFamily,
    /// **The operation's own kind.** Checkpoint K (a4), 2026-08-13.
    ///
    /// [`OperationFamily`] above is the *vendor-LUT* family the operation
    /// declares (eight values). It cannot distinguish `Adaptive3d` from
    /// 2D adaptive, nor `ProjectCurve` from `Trace` — and those are
    /// exactly the two kinds
    /// [`super::vendor_normalize::lut_query_for`] reroutes. Before a4 the
    /// Suggest side had no way to ask, so it queried the declared family
    /// while the gate queried the routed one.
    ///
    /// The constructor always writes `Some`. The `None` arms of the
    /// readers have no product reach (P29-7 7a finding).
    pub(in crate::feeds) operation_kind: Option<OperationType>,
    /// The vendor row key: the op's pass role. A row key, not a scope.
    pub(in crate::feeds) pass_role: PassRole,
    /// The stated depth (None = auto-calculate).
    pub(in crate::feeds) axial_depth_mm: Option<f64>,
    /// The stated width (None = auto-calculate).
    pub(in crate::feeds) radial_width_mm: Option<f64>,
    /// Target scallop height for ball/tapered ball finishing (mm).
    pub(in crate::feeds) target_scallop_mm: Option<f64>,
    /// The vendor table. The constructor writes `Some`;
    /// [`Self::with_no_vendor_table`] writes `None`.
    pub(in crate::feeds) vendor_lut: Option<&'a vendor_lut::VendorLut>,
    /// Physical setup context for feed derating.
    pub(in crate::feeds) setup: SetupContext,
    /// Spindle-RPM policy. See [`SpindleStrategy`].
    pub(in crate::feeds) spindle_strategy: SpindleStrategy,
    /// P21c (ruling Q11): the op's own feed cap
    /// (`DressupConfig::feed_max_rate`). Step 7 and the constant-chip XY
    /// limit read one ceiling,
    /// [`crate::machine::MachineProfile::op_cutting_ceiling`], at it.
    /// `None`: the machine ceiling alone. The constructor writes `None`
    /// (no product door states it today).
    pub(in crate::feeds) feed_max_rate: Option<f64>,
    /// P29-7 7c: the declared cuts of the op on the tool
    /// ([`planned::DeclaredCuts::of_operation`]). Step 6 and the published
    /// `power_kw` read the largest power over them
    /// ([`planned::worst_power`]); the lift reads their full-feed width.
    pub(in crate::feeds) cuts: planned::DeclaredCuts,
    /// P13a (review H3): the kinds of the tool facts that the calculator
    /// reads. An `Unknown` fact plans at its legacy row, and the basis of
    /// each value that reads the fact names that row. The constructor reads
    /// the tool (`sourced::ToolFactKinds::of_tool`).
    pub(in crate::feeds) tool_facts: sourced::ToolFactKinds,
    /// P29-7: the facts of the op's body cut
    /// (`operation.cuts(..).body_facts()`). Step 5 reads the thinning rule
    /// from it.
    pub(in crate::feeds) body_facts: CutFacts,
    #[expect(
        dead_code,
        reason = "the door is never read: it exists so that no code outside this module can build a literal"
    )]
    door: Door,
}

impl<'a> FeedsInput<'a> {
    /// The calculator input of `operation` on `tool`.
    ///
    /// The operation fills the row key (`OperationConfig::feeds_style`, the
    /// kind), the stated depth, width and scallop (`CutSpec::calc_inputs`),
    /// the declared cuts and the body facts. The tool fills the tool facts,
    /// the body, the series and the stick-out. The caller gives the
    /// material, the machine, the vendor table and the spindle policy.
    #[must_use]
    pub fn of_operation(
        operation: &OperationConfig,
        tool: &'a ToolConfig,
        material: &'a Material,
        machine: &'a MachineProfile,
        lut: &'a VendorLut,
        spindle_strategy: SpindleStrategy,
    ) -> Self {
        let (family, role) = operation.feeds_style();
        let (axial_hint, radial_hint, scallop_hint) =
            operation.cut_spec().calc_inputs(tool.diameter);
        let cuts = planned::DeclaredCuts::of_operation(
            operation,
            tool,
            &crate::feeds::entry_rule::ConfiguredEntry::registry_default(operation),
        );
        Self {
            // Ruling 2026-10-31: the cut direction declares the cutter series;
            // `UpCut` is a generic tool.
            tool_series: series::ToolSeries::for_cut_direction(tool.cut_direction),
            tool_diameter: tool.diameter,
            flute_count: tool.flute_count_planning(),
            flute_length: tool.cutting_length_planning_mm(),
            shank_diameter: Some(tool.shank_diameter_planning_mm()),
            // P22-0c-6b (ruling 4 (a)): the cutting cap and the core from the
            // body that `build_cutter` builds.
            cut_body: CutBody::of_tool(tool),
            tool_geometry: tool.geometry_hint(),
            material,
            machine,
            operation: family,
            // Checkpoint K (a4): the operation's own kind, so Suggest and the
            // gate route the LUT query through the same
            // `vendor_normalize::lut_query_for`.
            operation_kind: Some(operation.op_type()),
            pass_role: role,
            axial_depth_mm: axial_hint,
            radial_width_mm: radial_hint,
            target_scallop_mm: scallop_hint,
            vendor_lut: Some(lut),
            setup: SetupContext {
                tool_overhang_mm: Some(tool.stickout_planning_mm()),
            },
            feed_max_rate: None,
            spindle_strategy,
            cuts,
            tool_facts: sourced::ToolFactKinds::of_tool(tool),
            body_facts: cuts.list().body_facts(),
            door: Door,
        }
    }

    /// This input with no vendor table: the formula path of the calculator
    /// (`support::RecipeRowLookup::NoLut`). A test of the formula law
    /// states it. No product door reaches this path: each one passes the
    /// embedded table.
    #[must_use]
    pub fn with_no_vendor_table(self) -> Self {
        Self {
            vendor_lut: None,
            ..self
        }
    }

    /// This input with the op feed cap `mm_min` (P21c, ruling Q11). No
    /// product door states the cap today (`DressupConfig::feed_max_rate`
    /// does not reach the calculator), so the test of the one Step 5c and
    /// Step 7 ceiling states it here. It is a machine ceiling, not a cut
    /// fact.
    #[must_use]
    pub fn with_feed_max_rate(self, mm_min: f64) -> Self {
        Self {
            feed_max_rate: Some(mm_min),
            ..self
        }
    }

    /// The material.
    #[must_use]
    pub fn material(&self) -> &'a Material {
        self.material
    }

    /// The tool diameter (mm).
    #[must_use]
    pub fn tool_diameter(&self) -> f64 {
        self.tool_diameter
    }

    /// The flute count that planning uses.
    #[must_use]
    pub fn flute_count(&self) -> u32 {
        self.flute_count
    }

    /// The cutting cap and the core of the built tool.
    #[must_use]
    pub fn cut_body(&self) -> CutBody {
        self.cut_body
    }

    /// The tool shape.
    #[must_use]
    pub fn tool_geometry(&self) -> ToolGeometryHint {
        self.tool_geometry
    }

    /// The cutter series (`None`: a generic tool).
    #[must_use]
    pub fn tool_series(&self) -> Option<series::ToolSeries> {
        self.tool_series
    }

    /// The vendor table (`None` after [`Self::with_no_vendor_table`]).
    #[must_use]
    pub fn vendor_lut(&self) -> Option<&'a VendorLut> {
        self.vendor_lut
    }

    /// The machine.
    #[must_use]
    pub fn machine(&self) -> &'a MachineProfile {
        self.machine
    }

    /// P16 (review D7): `true` when the simulation gives the depth into the
    /// stock: the op's row reads its depth field as an offset from the
    /// model surface (`AxialRule::SurfaceOffset`, Project Curve). The
    /// calculator then has no depth: no family default depth, no flute
    /// guard, no slot cap, no power figure, and one
    /// [`super::FeedsWarning::DepthJudgedAfterSimulation`].
    #[must_use]
    pub fn depth_after_simulation(&self) -> bool {
        self.operation_kind.is_some_and(|k| {
            k.capabilities().cut_rule.axial == crate::compute::catalog::AxialRule::SurfaceOffset
        })
    }

    /// The force line of the power model: `None` when the material has no
    /// force line, or when the depth is known only after the simulation
    /// (P16: no power figure without a depth).
    pub(crate) fn power_force_line(&self) -> Option<crate::material::force_line::ForceLine> {
        if self.depth_after_simulation() {
            return None;
        }
        self.material.force_line().ok()
    }

    /// P20d: `true` for a drill cycle (`OperationFamily::Drill`: Drill and
    /// Pin Drill, `Radial::Axial`). Its power section is the axial circle
    /// ([`super::section::EngagedSection::axial_plunge`]), and its power is
    /// advisory: Step 6 does not act on it.
    #[must_use]
    pub fn is_axial_plunge(&self) -> bool {
        self.operation == OperationFamily::Drill
    }
}
