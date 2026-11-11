//! Feeds contract P13c (operator ruling O1, 2026-11-05): the force budget
//! of a machine class and the depth it allows.
//!
//! The budget stands in for the frame stiffness. The frame deflects by
//! `F / k_frame`, so on one frame a cap on the lateral force is a cap on
//! the frame deflection. The repo prints no frame stiffness (step 0 B), but
//! it prints the slot cuts that the hobby charts command on Shapeoko-class
//! routers. The hobby budget is the LARGEST peak engaged-edge force of
//! those printed full-slot cuts ([`HOBBY_SLOT_POINTS`]): the charts command
//! that force on that machine class.
//!
//! The roughing depth of the Pocket and Profile families is the shallowest
//! depth where the peak resultant at the cut width and chip meets the
//! budget ([`depth_at_budget`]). An industrial machine has no default
//! budget: its table states one (the P11c class rule).
//!
//! Every force here is [`Engagement::edge_force`], the one engaged-edge
//! model of the deflection and power readers (K7, P11a).

use std::sync::LazyLock;

use crate::feeds::ToolGeometryHint;
use crate::feeds::engagement::{EngagedDiameter, Engagement};
use crate::feeds::envelope::CutPoint;
use crate::material::{FormulaConstant, Material, SheetGoodKind, WoodSpecies};
use crate::tool::{MillingCutter, ToolDefinition};

/// One inch (mm): the charts print inch sizes and feeds.
const INCH_MM: f64 = 25.4;

/// The material line a chart row is read with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotPointMaterial {
    /// `WoodSpecies::GenericSoftwood` (the Curti density law).
    GenericSoftwood,
    /// `WoodSpecies::GenericHardwood` (the Curti density law).
    GenericHardwood,
    /// `SheetGoodKind::Mdf` (the Goli 2018 line).
    Mdf,
}

impl SlotPointMaterial {
    /// The material of this line.
    #[must_use]
    pub const fn material(self) -> Material {
        match self {
            Self::GenericSoftwood => Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
            Self::GenericHardwood => Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
            Self::Mdf => Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        }
    }
}

/// One printed full-slot cut of a hobby chart.
///
/// The numbers are the printed values (inch values converted at 25.4 mm).
/// `line` is the chart line verbatim (trimmed); the sentry
/// `the_hobby_force_budget_recomputes_from_the_printed_rows_p13c` checks
/// that the file holds it at `line_no` and that it prints each number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HobbySlotPoint {
    /// A short id.
    pub id: &'static str,
    /// The chart file, relative to the repo root.
    pub path: &'static str,
    /// The line number of the row in `path` (1-based).
    pub line_no: usize,
    /// The row, verbatim and trimmed.
    pub line: &'static str,
    /// Where the chart says the cut is a full slot and how many flutes the
    /// tool has, when the row does not say it.
    pub slot_note: &'static str,
    /// The tool diameter (mm).
    pub diameter_mm: f64,
    /// The flute count.
    pub flutes: u32,
    /// The material line the row is read with.
    pub material: SlotPointMaterial,
    /// The slot depth (mm).
    pub ap_mm: f64,
    /// The feed (mm/min).
    pub feed_mm_min: f64,
    /// The spindle speed (rpm).
    pub rpm: f64,
}

impl HobbySlotPoint {
    /// The advance per tooth (mm): `feed / (rpm x Z)`.
    #[must_use]
    pub fn fz_mm(&self) -> f64 {
        self.operating_point().fz_mm()
    }

    /// P29-1: the operating point of the printed cut (the printed feed,
    /// RPM and flute count).
    fn operating_point(&self) -> crate::feeds::cut_load::OperatingPoint {
        crate::feeds::cut_load::OperatingPoint::planned(self.feed_mm_min, self.rpm, self.flutes)
    }

    /// The peak engaged-edge resultant (N) of the printed cut: a flat end
    /// mill of the printed diameter in a full slot (`ae = D`) at the
    /// printed depth and chip, with the repo default helix
    /// (`tool::DEFAULT_HELIX_DEG`; the charts print no helix). `None` when
    /// the material has no force line.
    #[must_use]
    pub fn peak_force_n(&self) -> Option<f64> {
        let line = self.material.material().force_line().ok()?;
        let d = self.diameter_mm;
        let engagement = Engagement::from_cut(
            CutPoint {
                ap_mm: self.ap_mm,
                ae_mm: d,
            },
            ToolGeometryHint::Flat,
            EngagedDiameter::of_hint(ToolGeometryHint::Flat, d, d, self.ap_mm),
            Some(crate::tool::DEFAULT_HELIX_DEG),
        );
        crate::feeds::cut_load::CutLoad::of_line(
            &engagement,
            self.operating_point(),
            None,
            Some(line),
        )
        .force()
        .ok()
        .map(|f| f.peak_resultant_n)
    }
}

const SIENCI: &str =
    "planning/extrapolation_2026-09-24/fetch/G10/sources/sienci_feeds_speeds_metric.txt";
const CARBIDE3D_S3: &str =
    "planning/extrapolation_2026-09-24/fetch/G10/sources/carbide3d_s3_feeds_250.txt";
/// The note that the Carbide 3D Shapeoko 3 chart is a 3-flute 1/4" end mill
/// at 100 % engagement (a full slot):
/// `planning/extrapolation_2026-09-24/fetch/G9/sources/community_2402_post1_announcement.txt:5`.
const CARBIDE3D_SLOT_NOTE: &str = "Shapeoko 3: \"3 flute .25\\\" endmill, 100% engagement\" \
     (G9/sources/community_2402_post1_announcement.txt:5)";
/// The note of a Sienci row: the "Stepdown - Slotting" column is a full
/// slot; the chart prints no flute count, so 2 flutes are assumed (the
/// step 0 research, `vendor_points.json`).
const SIENCI_SLOT_NOTE: &str = "column \"Stepdown - Slotting\" (a full slot); \
     flute count not printed, 2 assumed";

/// The printed hobby full-slot cuts (Sienci and Carbide 3D charts for
/// Shapeoko-class routers), the six points of the step 0 research
/// (`planning/feeds_contract_2026-11-03/research_p13_step0/vendor_points.json`).
/// Amana and Harvey are left out: they are not hobby-machine charts.
///
/// The Sienci "Softwood, Soft Plywood, MDF" row is read twice, on the
/// softwood line and on the MDF line, because the row names both. The
/// Sienci chart prints two lower speed tiers per material (rows 26, 50,
/// 117, 141); each gives a lower peak force than its full-speed row, so
/// they do not move the largest point.
pub const HOBBY_SLOT_POINTS: [HobbySlotPoint; 6] = [
    HobbySlotPoint {
        id: "sienci_quarter_softwood",
        path: SIENCI,
        line_no: 79,
        line: "1/4\" Flat End Mill UC                               2740        1370            2.85               5.4                   12.1              12000            2",
        slot_note: SIENCI_SLOT_NOTE,
        diameter_mm: 0.25 * INCH_MM,
        flutes: 2,
        material: SlotPointMaterial::GenericSoftwood,
        ap_mm: 5.4,
        feed_mm_min: 2740.0,
        rpm: 12000.0,
    },
    HobbySlotPoint {
        id: "sienci_quarter_mdf",
        path: SIENCI,
        line_no: 79,
        line: "1/4\" Flat End Mill UC                               2740        1370            2.85               5.4                   12.1              12000            2",
        slot_note: SIENCI_SLOT_NOTE,
        diameter_mm: 0.25 * INCH_MM,
        flutes: 2,
        material: SlotPointMaterial::Mdf,
        ap_mm: 5.4,
        feed_mm_min: 2740.0,
        rpm: 12000.0,
    },
    HobbySlotPoint {
        id: "sienci_quarter_hardwood",
        path: SIENCI,
        line_no: 170,
        line: "1/4\" Flat End Mill UC                               2740        1370            2.85               2.5                   5.6               12000            2",
        slot_note: SIENCI_SLOT_NOTE,
        diameter_mm: 0.25 * INCH_MM,
        flutes: 2,
        material: SlotPointMaterial::GenericHardwood,
        ap_mm: 2.5,
        feed_mm_min: 2740.0,
        rpm: 12000.0,
    },
    HobbySlotPoint {
        id: "carbide3d_s3_mdf",
        path: CARBIDE3D_S3,
        line_no: 66,
        line: "MDF                  .3\u{2033}    17000   3    2    80             30",
        slot_note: CARBIDE3D_SLOT_NOTE,
        diameter_mm: 0.25 * INCH_MM,
        flutes: 3,
        material: SlotPointMaterial::Mdf,
        ap_mm: 0.3 * INCH_MM,
        feed_mm_min: 80.0 * INCH_MM,
        rpm: 17000.0,
    },
    HobbySlotPoint {
        id: "carbide3d_s3_pine",
        path: CARBIDE3D_S3,
        line_no: 63,
        line: "Pine                 .4\u{2033}    21000   3.75 3.5 75              40",
        slot_note: CARBIDE3D_SLOT_NOTE,
        diameter_mm: 0.25 * INCH_MM,
        flutes: 3,
        material: SlotPointMaterial::GenericSoftwood,
        ap_mm: 0.4 * INCH_MM,
        feed_mm_min: 75.0 * INCH_MM,
        rpm: 21000.0,
    },
    HobbySlotPoint {
        id: "carbide3d_s3_mahogany",
        path: CARBIDE3D_S3,
        line_no: 64,
        line: "Mahogany             .1\u{2033}    18950   3.5 3     65             32",
        slot_note: CARBIDE3D_SLOT_NOTE,
        diameter_mm: 0.25 * INCH_MM,
        flutes: 3,
        material: SlotPointMaterial::GenericHardwood,
        ap_mm: 0.1 * INCH_MM,
        feed_mm_min: 65.0 * INCH_MM,
        rpm: 18950.0,
    },
];

/// The hobby budget and the point that sets it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HobbyBudget {
    /// The budget (N): the largest peak force of [`HOBBY_SLOT_POINTS`].
    pub force_n: f64,
    /// The index of the point that sets it.
    pub binding: usize,
}

static HOBBY_BUDGET: LazyLock<HobbyBudget> = LazyLock::new(|| {
    let mut best = HobbyBudget {
        force_n: 0.0,
        binding: 0,
    };
    for (i, p) in HOBBY_SLOT_POINTS.iter().enumerate() {
        if let Some(f) = p.peak_force_n()
            && f > best.force_n
        {
            best = HobbyBudget {
                force_n: f,
                binding: i,
            };
        }
    }
    best
});

/// The hobby force budget (N) and its binding point, computed once from
/// [`HOBBY_SLOT_POINTS`] through [`Engagement::edge_force`]. No number is
/// typed: the sentry recomputes it from the chart rows.
#[must_use]
pub fn hobby_budget() -> HobbyBudget {
    *HOBBY_BUDGET
}

/// The F201 row of the hobby budget: `Cited`, the chart row that sets it
/// (its file prints the row verbatim).
#[must_use]
pub fn hobby_budget_row() -> FormulaConstant {
    let b = hobby_budget();
    let p = HOBBY_SLOT_POINTS
        .get(b.binding)
        .unwrap_or(&HOBBY_SLOT_POINTS[0]);
    FormulaConstant {
        name: std::borrow::Cow::Borrowed("machine.RigidityProfile.hobby.force_budget_n"),
        value: b.force_n,
        source: crate::material::ConstantSource::Cited {
            path: p.path,
            printed: p.line,
        },
    }
}

/// The law of the force-budget depth (every surface prints it).
pub const FORCE_BUDGET_LAW: &str = "peak engaged-edge force <= the machine force budget";

/// The depth scan steps of [`depth_at_budget`]. The peak resultant of a
/// helical edge is not monotone in depth: it rises while a flute's
/// engaged segment grows and plateaus (or falls, as flutes cancel) until
/// the next flute joins, one wrap pitch `2 pi r / (Z tan helix)` deeper
/// (16 mm for D 6, Z 2, helix 30 degrees; 8.6 mm for D 3.175). A
/// numerical step, not a formula constant: 24 steps over an 18 mm flute is
/// 0.75 mm or less (the scan starts at the depth bound `ap0`, see
/// [`depth_at_budget`]), inside one wrap pitch. A crossing and recrossing
/// inside one step is missed, as in the deflection solver. The envelope
/// solves it at each width sample, so the count is a cost choice (D5).
pub const BUDGET_SCAN_STEPS: usize = 24;

/// The depth that a force budget allows at one width and chip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BudgetDepth {
    /// The shallowest depth where the peak resultant meets the budget (mm).
    Solved(f64),
    /// The peak at the smallest depth the calculator writes is already over
    /// the budget: the minimum depth ships, with a warning (Q6: never
    /// refuse).
    BelowFloor,
    /// No depth up to the flute length reaches the budget: no force limit
    /// below the flute.
    NoRootBelowFlute,
}

impl BudgetDepth {
    /// The depth ceiling (mm): the solved depth, the minimum depth
    /// (`MIN_AP_MM`) when below the floor, `None` when no depth reaches the
    /// budget.
    #[must_use]
    pub fn ceiling_mm(self) -> Option<f64> {
        match self {
            Self::Solved(mm) => Some(mm),
            Self::BelowFloor => Some(crate::feeds::MIN_AP_MM),
            Self::NoRootBelowFlute => None,
        }
    }
}

/// The peak engaged-edge resultant (N) of `tool` at `(ap, ae)` and the
/// advance per tooth `fz_mm`, on `material`.
#[must_use]
pub fn peak_force_n(
    tool: &ToolDefinition,
    material: &Material,
    ap_mm: f64,
    ae_mm: f64,
    fz_mm: f64,
) -> Option<f64> {
    // P29-1: the one load, at the stated chip, summed for the tool's flutes.
    crate::feeds::cut_load::CutLoad::of(
        &Engagement::from_tool_cut(CutPoint { ap_mm, ae_mm }, tool),
        crate::feeds::cut_load::OperatingPoint::of_chip(fz_mm, tool.flute_count),
        tool,
        material,
    )
    .force()
    .ok()
    .map(|f| f.peak_resultant_n)
}

/// P13c (O1): the shallowest depth where the peak engaged-edge resultant of
/// `tool` at the width `ae_mm` and the advance per tooth `fz_mm` meets
/// `budget_n`. The span is `[max(MIN_AP_MM, ap0), flute length]`, scanned in
/// [`BUDGET_SCAN_STEPS`] then bisected (the one first-crossing solver,
/// `cutter_constraints::shallowest_crossing`).
///
/// `None` when the force model cannot answer: no force line, no helix, no
/// width, no chip, no budget, or no flute length.
#[must_use]
pub fn depth_at_budget(
    tool: &ToolDefinition,
    material: &Material,
    ae_mm: f64,
    fz_mm: f64,
    budget_n: f64,
) -> Option<BudgetDepth> {
    let usable = ae_mm.is_finite()
        && ae_mm > 0.0
        && fz_mm.is_finite()
        && fz_mm > 0.0
        && budget_n.is_finite()
        && budget_n > 0.0;
    if !usable {
        return None;
    }
    let line = material.force_line().ok()?;
    let floor = crate::feeds::MIN_AP_MM;
    let upper = tool.length();
    if !(upper.is_finite() && upper > floor) {
        return None;
    }
    // A bound that needs no edge sum: each engaged element pushes at most
    // `Ks fz + F_edge` per mm, and at most `Z x ap` of edge is engaged, so
    // the peak resultant is at most `Z ap (Ks fz + F_edge)`. Below
    // `ap0 = budget / (Z (Ks fz + F_edge))` no depth can reach the budget,
    // so the scan starts there; when `ap0` is past the flute, no depth
    // below the flute reaches it.
    let per_mm =
        f64::from(tool.flute_count.max(1)) * (line.ks_n_per_mm2() * fz_mm + line.f_edge_n_per_mm());
    let ap0 = if per_mm.is_finite() && per_mm > 0.0 {
        budget_n / per_mm
    } else {
        floor
    };
    if ap0 >= upper {
        return Some(BudgetDepth::NoRootBelowFlute);
    }
    let lower = ap0.max(floor);
    let within = |ap: f64| peak_force_n(tool, material, ap, ae_mm, fz_mm).map(|f| f <= budget_n);
    match crate::feeds::cutter_constraints::shallowest_crossing(
        lower,
        upper,
        BUDGET_SCAN_STEPS,
        within,
    ) {
        // Past the budget at the scan start: only the floor can start
        // there (`ap0` is at or below the floor).
        Some(mm) if mm <= 0.0 => Some(BudgetDepth::BelowFloor),
        Some(mm) => Some(BudgetDepth::Solved(mm)),
        // A refusal at the scan start is no signal; with a usable force
        // there, no crossing means no root below the flute.
        None => within(lower).map(|_| BudgetDepth::NoRootBelowFlute),
    }
}

/// P13c (O3, O4): the width factor of the corner-coverage rule.
///
/// Take a sharp 90 degree inside corner at the origin, walls on the axes,
/// a tool of radius `r` and the stepover `s` between two offset loops.
///
/// 1. The tool centre of the outer loop runs at `r` from each wall; its
///    corner is `(r, r)`. Its tool cuts every point within `r` of that
///    path, so near the corner the uncut region starts at `x > 2r` and
///    `y > 2r`. The corner of that region is `(2r, 2r)`.
/// 2. The inner loop runs at `r + s`; its corner is `(r + s, r + s)`.
/// 3. The point `(2r, 2r)` is the uncut point farthest from the inner
///    loop. Its distance to the inner corner is `(s - r) sqrt 2` (for
///    `s > r`).
/// 4. No material is left when that distance is at most `r`:
///    `(s - r) sqrt 2 <= r`, so `s <= r (1 + 1/sqrt 2)`.
///
/// In diameters: `(1 + 1/sqrt 2) / 2` (0.854 D). The step 0 research
/// checked the bound by sampling (`force_width.py` `corner_gap`: no gap at
/// `s = r (1 + 1/sqrt 2) - 0.02 r`, a gap above).
#[must_use]
pub fn corner_coverage_width_factor() -> f64 {
    (1.0 + 1.0 / std::f64::consts::SQRT_2) / 2.0
}

/// The F201 row of the corner-coverage factor: `Cited`, the step 0 finding
/// that prints the bound (`FINDINGS.md` F). The value is the derivation of
/// [`corner_coverage_width_factor`], not a typed number.
#[must_use]
pub fn corner_coverage_row(name: &'static str) -> FormulaConstant {
    FormulaConstant {
        name: std::borrow::Cow::Borrowed(name),
        value: corner_coverage_width_factor(),
        source: crate::material::ConstantSource::Cited {
            path: "planning/feeds_contract_2026-11-03/research_p13_step0/FINDINGS.md",
            printed: "(1 + 1/sqrt 2) r = 0.854 D",
        },
    }
}

/// The law of the corner-coverage rule.
pub const CORNER_COVERAGE_LAW: &str =
    "ae <= (1 + 1/sqrt 2) r: an offset pocket corner leaves no uncut material";

/// P13c (O3): the assumptions of a computed width. No limit in the
/// envelope models chip evacuation or tool life, so a computed width (the
/// removal-rate pick) states both. One producer: the GUI why card, MCP
/// `get_suggest_rationale` and the CLI `suggest` print these strings.
pub const COMPUTED_WIDTH_ASSUMPTIONS: [&str; 2] = ["chips clear", "tool life not modelled"];

/// The one line every surface prints for [`COMPUTED_WIDTH_ASSUMPTIONS`].
/// It fits the 60-character face limit of the card (IC1) with its "⚠ ".
#[must_use]
pub fn computed_width_assumptions_line() -> String {
    format!("Width assumes: {}", COMPUTED_WIDTH_ASSUMPTIONS.join("; "))
}

/// The outcome of [`rigidity_depth_cap`] for a family that has a rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RigidityCapOutcome {
    /// The cap, with its inputs.
    Cap(crate::machine::RigidityDepthCap),
    /// The force budget allows every depth up to the flute length: no
    /// rigidity limit below the flute (an absence, never drawn as a depth).
    NoLimitBelowFlute,
    /// The force model cannot answer, and the clause says why.
    Unmodelled(&'static str),
}

/// The clause of a force-budget cap whose material has no force line.
pub const NO_FORCE_LINE: &str =
    "the material has no force line: the force budget cannot set a depth";
/// The clause of a force-budget cap with no usable width, chip or flute.
pub const NO_BUDGET_INPUT: &str =
    "no usable width, chip or flute length: the force budget cannot set a depth";
/// The clause of [`RigidityCapOutcome::NoLimitBelowFlute`].
pub const NO_BUDGET_LIMIT_BELOW_THE_FLUTE: &str = "no force-budget limit below the flute";

/// **The one producer of the rigidity depth cap** (S3, P13c O1).
///
/// - The Adaptive family: `adaptive_doc_factor x` the engaged diameter at
///   `depth_mm` (`geometry::depth_cap_diameter_mm`).
/// - Every other roughing family: the force-budget depth at the width
///   `ae_mm` and the advance per tooth `fz_mm` ([`depth_at_budget`]).
///
/// `None` when the family and role have no cap (a drill, a finish pass).
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the cap reads the machine, the op class, the tool, the material and the cut"
)]
pub fn rigidity_depth_cap(
    machine: &crate::machine::MachineProfile,
    family: crate::feeds::OperationFamily,
    pass_role: crate::feeds::PassRole,
    tool: &ToolDefinition,
    material: &Material,
    ae_mm: f64,
    fz_mm: f64,
    depth_mm: f64,
) -> Option<RigidityCapOutcome> {
    use crate::machine::{DepthCapRule, RigidityDepthCap};
    // EXEMPT(F-DepthRule)
    let rule = machine.rigidity.depth_cap_rule(family, pass_role)?;
    Some(match rule {
        DepthCapRule::Factor(factor) => RigidityCapOutcome::Cap(RigidityDepthCap::Factor {
            factor,
            diameter_mm: crate::feeds::geometry::depth_cap_diameter_mm(tool, depth_mm),
        }),
        DepthCapRule::ForceBudget(budget_n) => {
            if material.force_line().is_err() {
                return Some(RigidityCapOutcome::Unmodelled(NO_FORCE_LINE));
            }
            match depth_at_budget(tool, material, ae_mm, fz_mm, budget_n) {
                None => RigidityCapOutcome::Unmodelled(NO_BUDGET_INPUT),
                Some(BudgetDepth::NoRootBelowFlute) => RigidityCapOutcome::NoLimitBelowFlute,
                Some(d @ (BudgetDepth::Solved(_) | BudgetDepth::BelowFloor)) => {
                    RigidityCapOutcome::Cap(RigidityDepthCap::ForceBudget {
                        budget_n,
                        ae_mm,
                        fz_mm,
                        depth_mm: d.ceiling_mm().unwrap_or(crate::feeds::MIN_AP_MM),
                        below_floor: matches!(d, BudgetDepth::BelowFloor),
                    })
                }
            }
        }
    })
}

/// The width and the advance per tooth that the rigidity cap of `op` is
/// solved at: the stated stepover (D for a slot), and the stated chip
/// (feed / (rpm x Z)), else the chip the calculator ships for the op's
/// cut. `None` when the op states no width or the calculator ships no
/// feed.
#[must_use]
pub fn cap_cut_of_operation(
    op: &crate::compute::catalog::OperationConfig,
    tool_cfg: &crate::compute::tool_config::ToolConfig,
    material: &Material,
    machine: &crate::machine::MachineProfile,
) -> Option<(f64, f64)> {
    use crate::compute::catalog::Radial;
    let d = tool_cfg.diameter;
    let ae = match op.cut_spec().radial {
        Radial::Width(Some(s)) => s.mm(),
        Radial::Slot | Radial::Axial => d,
        Radial::Width(None)
        | Radial::Scallop(_)
        | Radial::Groove
        | Radial::AngularStep(_)
        | Radial::SurfaceFollow => return None,
    };
    let z = f64::from(tool_cfg.flute_count_planning().max(1));
    let stated = match (op.feed_rate(), op.spindle_rpm()) {
        (Some(f), Some(rpm)) if f.is_finite() && f > 0.0 && rpm > 0 => {
            Some(f / (f64::from(rpm) * z))
        }
        _ => None,
    };
    let fz = match stated {
        Some(fz) => fz,
        None => {
            let input = crate::feeds::suggest::feeds_input_for_operation(
                op,
                tool_cfg,
                material,
                machine,
                crate::feeds::embedded_vendor_lut(),
                crate::feeds::SpindleStrategy::default(),
            );
            let r = crate::feeds::calculate(&input);
            (r.rpm > 0.0).then(|| r.feed_rate_mm_min / (r.rpm * z))?
        }
    };
    (ae.is_finite() && ae > 0.0 && fz.is_finite() && fz > 0.0).then_some((ae, fz))
}

/// The law of the force-budget edge where some width has no depth that
/// meets the budget (the minimum depth ships there, Q6).
pub const FORCE_BUDGET_FLOOR_LAW: &str =
    "peak engaged-edge force <= the machine force budget (minimum depth where none meets it)";

/// The warning every surface prints when no depth meets the force budget
/// at some width: the minimum depth ships (Q6: never refuse).
pub const FORCE_BUDGET_FLOOR_NOTE: &str = "No depth meets the machine force budget at some \
     width: the envelope ships the minimum depth there";

/// The F201 row name of the corner-coverage width factor.
pub const CORNER_COVERAGE_ROW_NAME: &str = "feeds.force_budget.corner_coverage_width_factor";

/// The short face of [`computed_width_assumptions_line`] for a narrow GUI
/// row (it fits at 340 points with its "⚠ "). The GUI shows the whole core
/// line on hover; MCP and the CLI print the core line.
pub const COMPUTED_WIDTH_ASSUMPTIONS_FACE: &str = "Assumes chips clear, no wear model";

/// The short face of [`FORCE_BUDGET_FLOOR_NOTE`] for a narrow GUI row.
pub const FORCE_BUDGET_FLOOR_FACE: &str = "No depth meets the force budget";

/// The short face of one envelope note (`SafeEnvelope::notes`): the face of
/// a known note, else the note itself.
#[must_use]
pub fn note_face(note: &str) -> String {
    if note == computed_width_assumptions_line() {
        COMPUTED_WIDTH_ASSUMPTIONS_FACE.to_owned()
    } else if note == FORCE_BUDGET_FLOOR_NOTE {
        FORCE_BUDGET_FLOOR_FACE.to_owned()
    } else {
        note.to_owned()
    }
}
