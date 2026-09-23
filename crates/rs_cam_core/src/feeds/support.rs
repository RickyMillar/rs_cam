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
//! makes the declaration visible. It does not refuse a new cell: every
//! registry row declares a formula source, so the `Refuse` arm is not
//! reachable in production. Ruling R1 decides when a cell may refuse.

use super::vendor_lookup::{self, LookupQuery, LookupResult};
use super::{FeedsInput, OperationFamily, VendorLut, vendor_normalize};

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

/// The source of the drill chipload in step 2 of [`super::calculate`]:
/// the milling formula times `DRILL_CHIPLOAD_MULTIPLIER`.
///
/// CREDITS.md, "Drill-subsystem provenance", declares the multiplier
/// unsourced. The LUT has no drill rows, so every drill cell uses this
/// formula.
pub const DRILL_FORMULA_SOURCE: &str = "repo-derived chipload formula k0*D^p*(1/H)^q \
     x DRILL_CHIPLOAD_MULTIPLIER 2.5 (feeds::calculate step 2); CREDITS.md \
     'Drill-subsystem provenance' declares the multiplier unsourced";

/// The reason on the `Refuse` arm. The error that carries it,
/// [`super::FeedsError::Unbacked`], names the operation, the tool family
/// and the material beside it.
pub const NO_BASIS_REASON: &str =
    "no vendor row matches this cell and the operation declares no formula source";

/// The basis Suggest has for one (operation, tool family, material
/// family) cell.
///
/// `VendorBacked` means that the recipe resolver found a row. A row that
/// publishes an RPM and no chipload (an RPM-only anchor) is still
/// `VendorBacked`: the RPM comes from the row, and
/// [`super::ChiploadSource::FormulaFallback`] records that the chipload
/// does not. Do not change this arm to follow the chipload source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedsSupport {
    /// A vendor row answered for this cell; the LUT is the basis.
    VendorBacked,
    /// No row; the calculator's formula answers, and this names its source.
    FormulaOnly { source: &'static str },
    /// The engine has no basis. Suggest refuses with the reason.
    Refuse { reason: &'static str },
}

/// The result of the recipe row lookup that [`super::calculate`] does.
pub(crate) enum RecipeRowLookup<'a> {
    /// The input carries no LUT.
    NoLut,
    /// `vendor_normalize::to_lookup_query` refused the routing (today, a
    /// `ProjectCurve` on a bull-nose, V-bit or facing cutter).
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
/// - The recipe row lookup finds a row: `VendorBacked`.
/// - No row, and the operation declares a formula source: `FormulaOnly`.
/// - No row, and no formula source: `Refuse`.
///
/// "No row" includes an input with no LUT and a routing refusal.
#[must_use]
pub fn feeds_support(input: &FeedsInput) -> FeedsSupport {
    support_for_lookup(input, &recipe_row_lookup(input))
}

/// The arm for a lookup that the caller already did. `calculate` uses
/// this so that it does the lookup one time.
pub(crate) fn support_for_lookup(input: &FeedsInput, lookup: &RecipeRowLookup) -> FeedsSupport {
    if matches!(lookup, RecipeRowLookup::Row { .. }) {
        return FeedsSupport::VendorBacked;
    }
    match formula_source_for_input(input) {
        Some(source) => FeedsSupport::FormulaOnly { source },
        None => FeedsSupport::Refuse {
            reason: NO_BASIS_REASON,
        },
    }
}
