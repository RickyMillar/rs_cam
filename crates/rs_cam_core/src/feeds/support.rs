//! The feeds support declaration: the basis Suggest has for one cell.
//!
//! A cell is one operation, one tool family and one material family.
//! [`feeds_support`] joins two halves:
//!
//! - the static half, `OperationSpec::feeds_formula_source`, which says
//!   whether the calculator's formula may answer for the operation and
//!   names the source of that formula;
//! - the dynamic half, the vendor row lookup that [`super::calculate`]
//!   does. [`recipe_row_lookup`] is that lookup, and `calculate` calls it,
//!   so the arm and the row the calculator used cannot disagree.
//!
//! Phase 0 of the feeds-matrix programme (`planning/feeds_matrix_2026-09-23/`)
//! made the declaration visible. Ruling R1 (operator, 2026-09-23) decides
//! when a formula-only cell may ship: only where an agent judgement found a
//! published figure within 0.5x to 2x of the formula's chipload
//! (`FORMULA_BACKING.md`, `FORMULA_BACKING_v2.md`). [`formula_backing`] is
//! that judgement as a table; a wood cell with no vendor row and no
//! `Backed` entry refuses. Non-wood materials were not judged and keep the
//! formula, as before, except a drill: ruling B5 (G6) refuses every drill
//! cell that no drill claim serves, in every material.

use std::borrow::Cow;

use super::extrapolation::{Claim, DrillClaim, FamilyClaim, SizeBasis};
use super::vendor_lookup::{self, LookupQuery, LookupResult};
use super::vendor_lut::{MaterialFamily, ToolFamily};
use super::{FeedsInput, OperationFamily, PassRole, VendorLut, vendor_normalize};

/// The source of the milling chipload formula in step 2 of
/// [`super::calculate`]: `k0 · D^p · (1/H)^q` from
/// `machine::ChipLoadFormula`.
///
/// CREDITS.md, "Chipload column convention and scaling laws", records the
/// two exponents as repo regressions over the vendor charts. The
/// coefficient `k0` has no entry there; only a code comment
/// ("Soft wood baseline from Shapeoko empirical data") names its origin.
pub const MILLING_FORMULA_SOURCE: &str = "repo-derived chipload formula k0*D^p*(1/H)^q \
     (machine::ChipLoadFormula, feeds::calculate step 2); exponents per CREDITS.md \
     'Chipload column convention and scaling laws'; k0 uncited (code comment \
     'Shapeoko empirical data' only)";

/// The source of the drill chipload preview in step 2 of
/// [`super::calculate`]: the milling formula with no multiplier.
///
/// Ruling B5 (2026-09-24) deleted the unsourced drill multiplier (2.5).
/// The symbol stays, because the drill registry rows and FM0 name it. The
/// number is a preview only: Suggest refuses every drill cell that no G6
/// claim serves ([`support_for_lookup`]), so no drill recipe ships from
/// this formula.
pub const DRILL_FORMULA_SOURCE: &str = "repo-derived milling formula k0*D^p*(1/H)^q with no \
     drill multiplier (deleted, ruling B5); a preview only: Suggest refuses every drill cell \
     that no G6 claim serves";

/// The reason on the `Refuse` arm. The error that carries it,
/// [`super::FeedsError::Unbacked`], names the operation, the tool family
/// and the material beside it.
pub const NO_BASIS_REASON: &str =
    "no vendor row matches this cell and the operation declares no formula source";

/// The agent judgement of ruling R1 for one formula-only cell class
/// (`planning/feeds_matrix_2026-09-23/FORMULA_BACKING.md`, sections 2 and
/// 4; re-derived in `FORMULA_BACKING_v2.md` after R5).
///
/// `Backed`: a published figure for the same tool family, a comparable
/// operation family and the same wood lies within 0.5x to 2x of the
/// formula's pre-derate chipload. `Clueless`: no such figure, or the
/// formula lies outside that range; the reason names what was searched.
/// The operator's threshold ("clueless sounds right to me", 2026-09-23).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormulaBacking {
    Backed,
    Clueless { reason: &'static str },
}

pub const UNJUDGED_REASON: &str = "No agent judgement covers this tool family, operation family \
     and material, so the formula has no checked basis.";
const BALL_MDF: &str = "No published figure backs the formula for a ball-nose cutter on adaptive, \
     pocket, contour or trace passes in MDF: the 1/4 in charts put it below half.";
const BALL_PLY: &str = "No published chipload exists for a ball-nose cutter on adaptive, pocket, \
     contour or trace passes in plywood.";
/// The refusal of a bull-nose finish pass in plywood (A3 step 4, G3). The
/// Amana corner-radius chart has no plywood column, so no family rule
/// serves this cell.
pub const BULL_FINISH_PLYWOOD: &str = "No published plywood chipload exists for a bull-nose \
     cutter on a finish pass: the Amana corner-radius chart prints softwood, hardwood and MDF \
     only.";
const VBIT_ADAPTIVE: &str = "No published figure exists for a V-bit on adaptive clearing, and the \
     recipe depth passes the end of the cone.";
const VBIT_PARALLEL: &str = "No published figure backs the formula for a V-bit on parallel finish \
     passes: at the engaged width the chip is below half of every V-bit figure.";
const VBIT_CONTOUR_FINISH: &str = "No published figure backs the formula for a V-bit on waterline \
     or steep-shallow passes: no vendor prints a V-bit 3D finish chart.";
const VBIT_MDF_PLY: &str = "No published figure backs the formula for a V-bit on pocket, contour \
     or trace passes in MDF or plywood: for a 1/4 in V-bit the formula is below half of the Onsrud \
     37-series bands (0.41x to 0.48x).";
/// The refusal of a V-bit on pocket, contour or trace passes in hardwood
/// (operator ruling 2026-09-25, "Hardwood V-bit formula-only cells:
/// refuse"). Face, Pocket, Profile, Rest and Zigzag always land here: no
/// wood chart prints a hardwood row for any of them, and the only printed
/// witness is the Onsrud V-bit band, which the formula is 0.37x to 0.57x
/// of. A Trace-family cell (VCarve, Trace, Inlay, or a `ProjectCurve`)
/// can also land here, but only at an angle no hardwood chart prints
/// (the printed angles are 15/18/30/45/60/90/120 deg); at a printed angle
/// it reads the row instead, `VendorBacked`. Since a second ruling the
/// same day ("a ProjectCurve on a V-bit routes ... as a trace"),
/// `ProjectCurve` reaches this arm exactly like any other Trace-family
/// cell — through the row lookup, not through a routing refusal.
const VBIT_HARDWOOD: &str = "No published figure backs the formula for a V-bit on pocket, \
     contour or trace passes in hardwood: the formula is 0.37x (6.35 mm) to 0.57x (12.7 mm) of \
     the Onsrud V-bit band, the only printed witness, and no printed hardwood row serves this \
     operation (operator ruling 2026-09-25).";
/// The refusal of a flat end mill plunge that the G6 drill claim does not
/// serve (ruling B5, range widened 2026-09-25): a diameter outside
/// 3.0-12.7 mm, a flute count other than 2 or 3, or a material with no
/// Spektra row.
pub const DRILL_FLAT: &str = "No published figure for this flat end mill plunge: the Amana \
     Spektra Ramp Down claim covers 3.0-12.7 mm, 2 or 3 flutes, in wood, plywood and MDF (G6 \
     drill, ruling B5).";
const DRILL_BALL: &str = "No published figure exists for a plunge drill with a ball-nose cutter \
     (G6 drill, ruling B5).";
/// The refusal of a bull-nose plunge (ruling B5, orchestrator decision 4).
pub const DRILL_BULL: &str = "No published figure exists for a plunge drill with a bull-nose \
     cutter: the one printed bull plunge (PreciseBits) is a feed with no RPM for one 3-flute \
     tool (G6 drill, ruling B5).";
const DRILL_VBIT: &str = "No published figure exists for a plunge drill with a V-bit, and a V-bit \
     cuts a cone, not a bore (G6 drill, ruling B5).";
const DRILL_TAPER: &str = "No published figure exists for a plunge drill with a tapered \
     ball-nose (G6 drill, ruling B5).";
const DRILL_OTHER: &str = "No published figure exists for a plunge drill with this cutter (G6 \
     drill, ruling B5).";

/// Ruling R1 applied to size (operator, 2026-09-24): a tool whose lookup
/// diameter is under this value (mm) is a micro tool. A micro tool ships only
/// from a row within [`MICRO_ROW_RATIO_MIN`] to [`MICRO_ROW_RATIO_MAX`] of
/// its own diameter.
///
/// The probe that found it: tapered ball tips of 0.5 / 0.8 / 1.0 mm on a
/// Scallop shipped 0.0347 / 0.0454 / 0.0520 mm/tooth (6.9 / 5.7 / 5.2 % of
/// the tip), scaled 6-8x down from the 1/8 in and 1/4 in Onsrud 77-100 rows by
/// the `(d/D)^0.61` law, which was fitted on 3-12 mm rows. No chart publishes
/// a figure at that size, so the engine has no basis.
pub const MICRO_TOOL_DIAMETER_MM: f64 = 1.5;

/// The largest row-diameter / tool-diameter ratio a micro tool may ship from.
/// A row more than 2x the tool is an extrapolation the law was not fitted on.
pub const MICRO_ROW_RATIO_MAX: f64 = 2.0;

/// The smallest row-diameter / tool-diameter ratio a micro tool may ship
/// from (a row under half the tool).
pub const MICRO_ROW_RATIO_MIN: f64 = 0.5;

/// Ruling B1 (operator, 2026-09-24): the smallest tapered-ball tip (mm) that
/// a wood chart prints. No wood chart prints a tip under this value, so
/// Suggest refuses a tapered ball whose lookup key (its tip, ruling A1) is
/// under it ([`tapered_tip_floor_refusal`]).
pub const TAPERED_MIN_TIP_MM: f64 = 0.5;

/// The tip floor on a tapered ball: `Some(reason)` when the tool is a
/// tapered ball nose and its lookup key is under [`TAPERED_MIN_TIP_MM`].
/// The key of a tapered ball is its tip (ruling A1), so the text names the
/// tip. A tip of exactly 0.5 mm passes to the size rule
/// ([`micro_extrapolation_refusal`]).
#[must_use]
pub fn tapered_tip_floor_refusal(tool: ToolFamily, lookup_diameter_mm: f64) -> Option<String> {
    if tool != ToolFamily::TaperedBallNose
        || !lookup_diameter_mm.is_finite()
        || lookup_diameter_mm >= TAPERED_MIN_TIP_MM
    {
        return None;
    }
    Some(format!(
        "no published figure for a {lookup_diameter_mm:.2} mm tapered ball nose: no wood chart \
         prints a tip under {TAPERED_MIN_TIP_MM} mm (ruling B1)"
    ))
}

/// The size rule on a matched row: `Some(reason)` when the tool is a micro
/// tool and the row is outside the ratio window. Rows with no diameter
/// (an angle-keyed V-bit row, a diameter-window article) are not judged by
/// size.
///
/// The rule is keyed on the LOOKUP diameter (`lookup_diameter_mm`, the key
/// that the row lookup scales by), because that is the diameter the row's
/// chipload is transferred to. Since ruling A1 (2026-09-24) the key of a
/// tapered ball is its tip, and since ruling B4 (G5) the key of a V-bit is
/// its nominal diameter. So every production caller (the G1 size claim,
/// `extrapolation::size`) passes one diameter twice, and the text names no
/// engaged diameter. The text names the tool's nominal diameter with two
/// decimals, adds the key as "(engaged ...)" only when a caller passes two
/// different diameters (FM9 pins that form), and names the row's diameter
/// and the ratio: "no published figure for a 0.50 mm tapered ball nose; the
/// nearest chart row is 3.175 mm, 6.3x the tool, ...".
#[must_use]
pub fn micro_extrapolation_refusal(
    tool: ToolFamily,
    nominal_diameter_mm: f64,
    lookup_diameter_mm: f64,
    row_diameter_mm: f64,
) -> Option<String> {
    if !(lookup_diameter_mm.is_finite() && lookup_diameter_mm > 0.0)
        || !(row_diameter_mm.is_finite() && row_diameter_mm > 0.0)
        || lookup_diameter_mm >= MICRO_TOOL_DIAMETER_MM
    {
        return None;
    }
    let ratio = row_diameter_mm / lookup_diameter_mm;
    if (MICRO_ROW_RATIO_MIN..=MICRO_ROW_RATIO_MAX).contains(&ratio) {
        return None;
    }
    let engaged = if (lookup_diameter_mm - nominal_diameter_mm).abs() >= 0.005 {
        format!(" (engaged {lookup_diameter_mm:.2} mm at the cut depth)")
    } else {
        String::new()
    };
    Some(format!(
        "no published figure for a {nominal_diameter_mm:.2} mm {}{engaged}; the nearest chart \
         row is {row_diameter_mm} mm, {ratio:.1}x the tool, outside the {MICRO_ROW_RATIO_MIN}x \
         to {MICRO_ROW_RATIO_MAX}x window a tool under {MICRO_TOOL_DIAMETER_MM} mm needs \
         (ruling R1 applied to size)",
        tool.label()
    ))
}

/// The four wood families the judgement covered.
#[must_use]
pub fn is_judged_wood(material: MaterialFamily) -> bool {
    matches!(
        material,
        MaterialFamily::Softwood
            | MaterialFamily::Hardwood
            | MaterialFamily::Mdf
            | MaterialFamily::PlywoodHardwood
    )
}

/// The R1 judgement for a formula-only cell class. Only meaningful for a
/// judged wood family ([`is_judged_wood`]); the caller keeps the formula
/// for every other material. The drill arms are the exception: they ignore
/// the material, and the caller reads them for every material (ruling B5).
#[must_use]
pub fn formula_backing(
    tool: ToolFamily,
    family: OperationFamily,
    role: PassRole,
    material: MaterialFamily,
) -> FormulaBacking {
    use FormulaBacking::{Backed, Clueless};
    use MaterialFamily::{Hardwood, Mdf, PlywoodHardwood, Softwood};
    use OperationFamily::{Adaptive, Contour, Drill, Parallel, Pocket, Scallop, Trace};
    use PassRole::{Finish, Roughing, SemiFinish};
    let sw_hw = matches!(material, Softwood | Hardwood);
    let mdf_ply = matches!(material, Mdf | PlywoodHardwood);
    match (tool, family, role) {
        // Drill (G6, ruling B5): no row is filed under the drill family. The
        // drill claim (`extrapolation::drill`) serves a 2- or 3-flute flat
        // end mill at 3.0-12.7 mm from its Spektra side row; every other
        // drill cell reaches this table and refuses, in every material
        // (`support_for_lookup`).
        (ToolFamily::FlatEnd, Drill, _) => Clueless { reason: DRILL_FLAT },
        (ToolFamily::BallNose, Drill, _) => Clueless { reason: DRILL_BALL },
        (ToolFamily::BullNose, Drill, _) => Clueless { reason: DRILL_BULL },
        (ToolFamily::ChamferVbit, Drill, _) => Clueless { reason: DRILL_VBIT },
        (ToolFamily::TaperedBallNose, Drill, _) => Clueless {
            reason: DRILL_TAPER,
        },
        (_, Drill, _) => Clueless {
            reason: DRILL_OTHER,
        },
        // Flat end mill: Onsrud and Amana 1 x D charts back trace and parallel finishes.
        (ToolFamily::FlatEnd, Trace | Parallel, Finish) => Backed,
        // Ball nose: backed in the two solid woods, not in MDF or plywood.
        (ToolFamily::BallNose, Adaptive | Pocket, Roughing)
        | (ToolFamily::BallNose, Contour, _)
        | (ToolFamily::BallNose, Trace, Finish)
            if sw_hw =>
        {
            Backed
        }
        (ToolFamily::BallNose, Adaptive | Pocket, Roughing)
        | (ToolFamily::BallNose, Contour, _)
        | (ToolFamily::BallNose, Trace, Finish)
            if material == Mdf =>
        {
            Clueless { reason: BALL_MDF }
        }
        (ToolFamily::BallNose, Adaptive | Pocket, Roughing)
        | (ToolFamily::BallNose, Contour, _)
        | (ToolFamily::BallNose, Trace, Finish)
            if material == PlywoodHardwood =>
        {
            Clueless { reason: BALL_PLY }
        }
        // Bull nose, finish pass in plywood. Since A3 step 4 (G3) the Amana
        // corner-radius pocket rows serve the parallel, scallop and trace
        // families through the family rule (`extrapolation::family`) in
        // softwood, hardwood and MDF. The chart prints no plywood column, so
        // a plywood finish pass finds no row and refuses here. In the other
        // judged woods a cell with no row falls to the unjudged reason below.
        (ToolFamily::BullNose, Parallel | Scallop | Trace, Finish)
            if material == PlywoodHardwood =>
        {
            Clueless {
                reason: BULL_FINISH_PLYWOOD,
            }
        }
        // V-bit: no figure backs adaptive, parallel or 3D contour finish
        // passes in any judged wood. Pocket, contour and trace passes are
        // backed in softwood only (ruling R1). In hardwood the formula
        // falls under 0.5x of the Onsrud V-bit band, so these cells refuse
        // (operator ruling 2026-09-25). In MDF and plywood the Onsrud
        // 37-series rows (extrapolation P2, G2) put the formula below half
        // at 1/4 in (ruling R1, strict: decision 2).
        (ToolFamily::ChamferVbit, Adaptive, Roughing) => Clueless {
            reason: VBIT_ADAPTIVE,
        },
        (ToolFamily::ChamferVbit, Parallel, Finish) => Clueless {
            reason: VBIT_PARALLEL,
        },
        (ToolFamily::ChamferVbit, Contour, SemiFinish | Finish) => Clueless {
            reason: VBIT_CONTOUR_FINISH,
        },
        (ToolFamily::ChamferVbit, Pocket | Contour, Roughing)
        | (ToolFamily::ChamferVbit, Trace, Finish)
            if material == Softwood =>
        {
            Backed
        }
        (ToolFamily::ChamferVbit, Pocket | Contour, Roughing)
        | (ToolFamily::ChamferVbit, Trace, Finish)
            if material == Hardwood =>
        {
            Clueless {
                reason: VBIT_HARDWOOD,
            }
        }
        (ToolFamily::ChamferVbit, Pocket | Contour, Roughing)
        | (ToolFamily::ChamferVbit, Trace, Finish)
            if mdf_ply =>
        {
            Clueless {
                reason: VBIT_MDF_PLY,
            }
        }
        // Tapered ball: no formula arm since A3 (G3). The Onsrud 77-100
        // pocket rows serve the adaptive, contour, parallel, scallop and trace
        // families through the family rule (`extrapolation::family`), in all
        // four judged woods. A tip under 0.5 mm refuses on the tip floor
        // first; a cell with no row falls to the unjudged reason below.
        _ => Clueless {
            reason: UNJUDGED_REASON,
        },
    }
}

/// The basis Suggest has for one (operation, tool family, material
/// family) cell.
///
/// `VendorBacked` means that the recipe resolver found a row. A row that
/// publishes an RPM and no chipload (an RPM-only anchor) is still
/// `VendorBacked`: the RPM comes from the row, and
/// [`super::ChiploadSource::FormulaFallback`] records that the chipload
/// does not. Do not change this arm to follow the chipload source.
///
/// Not `Copy` since 2026-09-24: the size refusal builds its reason from the
/// two diameters, so the reason is a `Cow`. Not `Eq` since extrapolation
/// P1 step 3: a [`Claim`] holds `f64` values.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedsSupport {
    /// A vendor row answered for this cell at its printed size (or the row
    /// has no diameter, or it publishes an RPM and no chipload); the LUT is
    /// the basis.
    VendorBacked,
    /// A vendor row answered through a stated G1 size claim: the row is
    /// off the tool's size, and the claim names the rule, the range and the
    /// residual (`feeds::extrapolation`). The claim is boxed because it is
    /// much larger than the other variants.
    Extrapolated { claim: Box<Claim> },
    /// A vendor row answered through a stated G3 family claim: a family
    /// rule carries the row from its home operation family into this one
    /// (`feeds::extrapolation::family`, A3). `size` is the G1 size claim
    /// when the row is also off the tool's size, and `None` at the printed
    /// size.
    FamilyTransferred {
        family: Box<FamilyClaim>,
        size: Option<Box<Claim>>,
    },
    /// A vendor row answered through the G6 drill claim: the drill rule
    /// reads a flat end mill's printed side row for a plunge, and the chip
    /// is the side chip / Z (`feeds::extrapolation::drill`, ruling B5).
    /// `size` is the G1 size claim when the row is also off the tool's
    /// size, and `None` at the printed size.
    DrillTransferred {
        drill: Box<DrillClaim>,
        size: Option<Box<Claim>>,
    },
    /// No row; the calculator's formula answers, and this names its source.
    FormulaOnly { source: &'static str },
    /// The engine has no basis. Suggest refuses with the reason.
    Refuse { reason: Cow<'static, str> },
}

impl FeedsSupport {
    /// The card text of the arm: a headline and a detail line, in plain
    /// words. The MCP and the FM1 instrument read it.
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        match self {
            Self::VendorBacked => (
                "vendor-backed".to_owned(),
                "a vendor row prints this cell".to_owned(),
            ),
            Self::Extrapolated { claim } => claim.card_text(),
            // The two claims: the headlines joined, then the details joined.
            Self::FamilyTransferred { family, size } => {
                join_size_claim(family.card_text(), size.as_deref())
            }
            Self::DrillTransferred { drill, size } => {
                join_size_claim(drill.card_text(), size.as_deref())
            }
            Self::FormulaOnly { source } => ("formula only".to_owned(), (*source).to_owned()),
            Self::Refuse { reason } => ("refused".to_owned(), reason.to_string()),
        }
    }
}

/// A claim's card text, joined with the size claim's text when there is one:
/// the headlines joined, then the details joined.
fn join_size_claim((headline, detail): (String, String), size: Option<&Claim>) -> (String, String) {
    match size {
        None => (headline, detail),
        Some(claim) => {
            let (size_headline, size_detail) = claim.card_text();
            (
                format!("{headline}; {size_headline}"),
                format!("{detail}; {size_detail}"),
            )
        }
    }
}

/// The result of the recipe row lookup that [`super::calculate`] does.
pub(crate) enum RecipeRowLookup<'a> {
    /// The input carries no LUT.
    NoLut,
    /// `vendor_normalize::to_lookup_query` refused the routing (today, a
    /// `ProjectCurve` on a facing cutter; operator ruling 2026-09-25 routes
    /// a V-bit `ProjectCurve` to the printed Trace rows instead).
    RoutingRefused,
    /// The routed query ran and no row matched.
    NoRow,
    /// The routed query ran and this row matched. The row is boxed
    /// because it is much larger than the other variants.
    Row {
        lut: &'a VendorLut,
        query: LookupQuery,
        row: Box<LookupResult>,
    },
}

/// The one recipe row lookup: `find_best_row_for_geometry` on the query
/// that `vendor_normalize::to_lookup_query` routes. [`super::calculate`]
/// and [`feeds_support`] both call it.
pub(crate) fn recipe_row_lookup<'a>(input: &FeedsInput<'a>) -> RecipeRowLookup<'a> {
    let Some(lut) = input.vendor_lut else {
        return RecipeRowLookup::NoLut;
    };
    let Some(query) = vendor_normalize::to_lookup_query(input) else {
        return RecipeRowLookup::RoutingRefused;
    };
    match vendor_lookup::find_best_row_for_geometry(lut, &query, &input.tool_geometry) {
        Some(row) => RecipeRowLookup::Row {
            lut,
            query,
            row: Box::new(row),
        },
        None => RecipeRowLookup::NoRow,
    }
}

/// The formula source the calculator cites for an operation family when
/// the input names no operation: the drill formula for `Drill`, the
/// milling formula for every other family.
#[must_use]
pub fn calculator_formula_source(family: OperationFamily) -> &'static str {
    if family == OperationFamily::Drill {
        DRILL_FORMULA_SOURCE
    } else {
        MILLING_FORMULA_SOURCE
    }
}

/// The static half of the declaration for this input.
///
/// With an operation kind, this is the registry row's
/// `feeds_formula_source`. Without one (a raw `calculate` call), this is
/// the calculator's own citation for the input's family.
#[must_use]
pub fn formula_source_for_input(input: &FeedsInput) -> Option<&'static str> {
    match input.operation_kind {
        Some(kind) => kind.spec().feeds_formula_source,
        None => Some(calculator_formula_source(input.operation)),
    }
}

/// The support arm for the cell this input describes.
///
/// - A tapered ball whose tip is under [`TAPERED_MIN_TIP_MM`]: `Refuse`
///   ([`tapered_tip_floor_refusal`], ruling B1), with or without a row.
/// - The recipe row lookup finds a row: the row's G1 size basis
///   (`LookupResult::size_basis`) decides. A claim gives `Extrapolated`; a
///   refusal gives `Refuse` with the claim's reason (for a tool under
///   1.5 mm that text is [`micro_extrapolation_refusal`]); every other
///   basis (`Exact`, `NoDiameterAnchor`, `NoChipload`, the V-bit
///   `AngleKey` of ruling B4) gives `VendorBacked`. Then, when a G3 family rule carries the row
///   (`LookupResult::family_basis`), `VendorBacked` becomes
///   `FamilyTransferred { size: None }` and `Extrapolated` becomes
///   `FamilyTransferred { size: Some(claim) }`. A size `Refuse` stays.
///   In the same way, when the G6 drill rule reads the row
///   (`LookupResult::drill_basis`), the arm becomes `DrillTransferred`.
/// - No row on a drill cell: `Refuse` with the [`formula_backing`] drill
///   reason, in every material and with or without an operation kind
///   (ruling B5, orchestrator decision 3).
/// - No row, and no formula source: `Refuse`.
/// - No row, a formula source, an operation kind and a judged wood
///   material: [`formula_backing`] decides, `Backed` -> `FormulaOnly`,
///   `Clueless` -> `Refuse` (ruling R1).
/// - No row and a formula source otherwise (no operation kind, or a
///   material the judgement did not cover): `FormulaOnly`, as before.
///
/// "No row" includes an input with no LUT and a routing refusal.
#[must_use]
pub fn feeds_support(input: &FeedsInput) -> FeedsSupport {
    support_for_lookup(input, &recipe_row_lookup(input))
}

/// The arm for a lookup that the caller already did. `calculate` uses
/// this so that it does the lookup one time.
pub(crate) fn support_for_lookup(input: &FeedsInput, lookup: &RecipeRowLookup) -> FeedsSupport {
    // Ruling B1: the tapered tip floor runs first, whether a row matched or
    // not. With a row, a 0.3 mm tip against the 0.5 mm row is 1.67x, inside
    // the size rule's window, so the size rule alone lets it through. With
    // no row (a 0.3 mm Scallop is under 0.1x of every Scallop row), the
    // formula judgement would otherwise ship a formula number. The lookup
    // key of a tapered ball is its tip (`geometry::lut_key_diameter_mm`,
    // ruling A1), which is `input.tool_diameter`.
    let tool = input.tool_geometry.cutter_kind().lut_family();
    let key = super::geometry::lut_key_diameter_mm(
        input.tool_geometry,
        input.axial_depth_mm.unwrap_or(input.tool_diameter),
        input.tool_diameter,
        input.shank_diameter.unwrap_or(input.tool_diameter),
    );
    if let Some(reason) = tapered_tip_floor_refusal(tool, key) {
        return FeedsSupport::Refuse {
            reason: Cow::Owned(reason),
        };
    }
    if let RecipeRowLookup::Row { row, .. } = lookup {
        let arm = match &row.size_basis {
            SizeBasis::Claim(claim) => FeedsSupport::Extrapolated {
                claim: claim.clone(),
            },
            // The claim's reason is the Suggest text. The lookup key is the
            // nominal diameter for every family (the tip of a tapered ball,
            // ruling A1; the nominal diameter of a V-bit, ruling B4), so
            // the claim already names the tool's own size.
            SizeBasis::Refused { reason, .. } => FeedsSupport::Refuse {
                reason: Cow::Owned(reason.clone()),
            },
            SizeBasis::Exact
            | SizeBasis::NoDiameterAnchor
            | SizeBasis::NoChipload
            | SizeBasis::AngleKey { .. } => FeedsSupport::VendorBacked,
        };
        // A3 (G3): a transferred row states its family claim beside the size
        // arm. A size refusal still wins: a refused row has no band.
        if let Some(family) = row.family_basis.claim() {
            return match arm {
                FeedsSupport::VendorBacked => FeedsSupport::FamilyTransferred {
                    family: Box::new(family.clone()),
                    size: None,
                },
                FeedsSupport::Extrapolated { claim } => FeedsSupport::FamilyTransferred {
                    family: Box::new(family.clone()),
                    size: Some(claim),
                },
                other @ (FeedsSupport::Refuse { .. }
                | FeedsSupport::FormulaOnly { .. }
                | FeedsSupport::FamilyTransferred { .. }
                | FeedsSupport::DrillTransferred { .. }) => other,
            };
        }
        // B5 (G6): a row that the drill rule reads states its drill claim
        // beside the size arm, in the same shape as G3. A size refusal still
        // wins. No family rule serves the drill family, so the two remaps
        // never meet on one row.
        if let Some(drill) = row.drill_basis.claim() {
            return match arm {
                FeedsSupport::VendorBacked => FeedsSupport::DrillTransferred {
                    drill: Box::new(drill.clone()),
                    size: None,
                },
                FeedsSupport::Extrapolated { claim } => FeedsSupport::DrillTransferred {
                    drill: Box::new(drill.clone()),
                    size: Some(claim),
                },
                other @ (FeedsSupport::Refuse { .. }
                | FeedsSupport::FormulaOnly { .. }
                | FeedsSupport::FamilyTransferred { .. }
                | FeedsSupport::DrillTransferred { .. }) => other,
            };
        }
        return arm;
    }
    // Ruling B5 (G6, orchestrator decision 3): a drill cell that no drill
    // claim serves refuses, in every material. Without the drill multiplier
    // the formula is an unjudged milling number on a plunge, so plastics,
    // aluminium and a raw `calculate` with no operation kind refuse too.
    // `calculate` still computes the formula preview; the arm says it has
    // no basis.
    if input.operation == OperationFamily::Drill {
        let material = vendor_normalize::material_to_lut(input.material).0;
        let reason = match formula_backing(tool, OperationFamily::Drill, input.pass_role, material)
        {
            FormulaBacking::Clueless { reason } => reason,
            // Every drill arm of the judgement is `Clueless`; this arm
            // keeps the refusal if a future arm says otherwise.
            FormulaBacking::Backed => DRILL_OTHER,
        };
        return FeedsSupport::Refuse {
            reason: Cow::Borrowed(reason),
        };
    }
    let Some(source) = formula_source_for_input(input) else {
        return FeedsSupport::Refuse {
            reason: Cow::Borrowed(NO_BASIS_REASON),
        };
    };
    if input.operation_kind.is_none() {
        return FeedsSupport::FormulaOnly { source };
    }
    let material = vendor_normalize::material_to_lut(input.material).0;
    if !is_judged_wood(material) {
        return FeedsSupport::FormulaOnly { source };
    }
    match formula_backing(tool, input.operation, input.pass_role, material) {
        FormulaBacking::Backed => FeedsSupport::FormulaOnly { source },
        FormulaBacking::Clueless { reason } => FeedsSupport::Refuse {
            reason: Cow::Borrowed(reason),
        },
    }
}
