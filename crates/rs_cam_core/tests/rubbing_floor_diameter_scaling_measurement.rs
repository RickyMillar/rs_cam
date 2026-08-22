//! **Should `RUBBING_FLOOR_MM_TOOTH` scale with tool diameter? — a MEASUREMENT,
//! not a change.**
//!
//! Nothing in this file moves a threshold, a band, a constant or a scaling
//! exponent. It measures the one that exists and writes down what the measurement
//! supports. [`the_constant_this_instrument_measured_has_not_moved`] pins that
//! promise so a later reader can tell whether the numbers below were taken at the
//! floor the crate still ships.
//!
//! # The question
//!
//! [`rs_cam_core::feeds::RUBBING_FLOOR_MM_TOOTH`] = 0.025 mm/tooth is the advance
//! below which the crate says a cutter rubs instead of cutting. It is applied by
//! [`rs_cam_core::feeds::effective_rubbing_floor`] as `min(floor, band_max)` when a
//! vendor band was matched, and as the **bare constant** when none was — and it
//! carries no diameter and no material, while every other chipload quantity in the
//! crate carries both:
//!
//! | quantity | site | diameter law |
//! |---|---|---|
//! | vendor band transfer | `vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT` | `D^0.61` |
//! | formula fallback | `machine::ChipLoadFormula::default().p` | `D^0.61` |
//! | **rubbing floor** | `feeds::RUBBING_FLOOR_MM_TOOTH` | **none** |
//!
//! Observed live 2026-08-21 on a Ø1.0-tip tapered ball: Suggest's own model asked
//! for 0.0093–0.0137 mm/tooth at realistic finishing depths and the floor overrode
//! it to 0.025 — 1.8x–2.7x up, on a tool where 0.025 mm/tooth is about 2.5 % of the
//! tip diameter. On a Ø6 cutter the same constant is about 0.4 % of diameter.
//!
//! # What is measured here, and by which instrument
//!
//! Five measurements. Three of them need no fixture at all, because they read
//! shipped data rather than a synthetic cut.
//!
//! 1. [`report_how_often_the_rubbing_floor_binds`] — a Suggest sweep over
//!    16 cutters x 7 cut cases x 10 species x 3 machine presets. How often the
//!    clamp fires, by what factor it raises the advance, split by whether a vendor
//!    band was matched, and how often the machine feed cap stops it reaching the
//!    floor anyway.
//! 2. [`report_the_bound_floor_as_a_fraction_of_tool_diameter`] — the "2.5 % vs
//!    0.4 %" asymmetry, measured across a diameter span instead of asserted from
//!    two points.
//! 3. [`report_a_band_the_floor_never_saw`] — the counterfactual. For every
//!    unbanded row it re-runs the **envelope** resolver
//!    (`vendor_lookup::find_best_chip_envelope_row`, the one the post-sim gate
//!    uses) with the same query and the same DOC derate, and reports how often a
//!    chipload-bearing row existed that `effective_rubbing_floor` was never handed.
//! 4. [`report_vendor_published_chipload_minimum_versus_diameter`] — how the LUT's
//!    own published `chipload_min` scales with the row's published diameter, how
//!    many rows print a minimum below the floor, and the same count split by
//!    cutter family, which is the table that moved this file's proposal off a
//!    pure-diameter framing.
//! 5. [`report_the_literature_matrix_already_varies_its_own_floor`] — the ladder of
//!    per-cell floors the repo's literature matrix already writes down.
//!
//! # Non-vacuity
//!
//! This project has had four occurrences of a gate reading healthy on an empty
//! population. Every test here guards its own population **before** reporting a
//! statistic over it, and panics naming the axis to widen. A sweep with no
//! floor-binding rows, no banded rows or no unbanded rows is not a clean result; it
//! is an unmeasured one, and it fails.
//!
//! # PROPOSAL
//!
//! ## What the shipped data says, before any sweep runs
//!
//! Measured over the embedded LUT and `cells.toml` (tests 4 and 5 reproduce both
//! from live code; these figures were taken from the same files on 2026-08-22):
//!
//! - Of 170 wood rows that publish both a diameter and a `chipload_min`,
//!   **19 print a minimum below 0.025 mm/tooth**, and **5 print an entire band
//!   below it** — as published, before any diameter or hardness transfer. The
//!   constant is not a floor *beneath* the vendor data; it sits inside it, and
//!   forbids operating points the vendors print.
//! - **And the split is by cutter FAMILY at least as much as by diameter.** The
//!   family tally is the sharpest single number in this file:
//!
//!   | family / vendor op | rows | below 0.025 |
//!   |---|---:|---:|
//!   | `tapered_ball_nose` / parallel + scallop | 8 | **8 (100 %)** |
//!   | `ball_nose` / parallel | 17 | **9 (53 %)** |
//!   | `flat_end` / adaptive + contour | 56 | 2 (4 %) |
//!   | `flat_end` / pocket | 56 | **0** |
//!   | v-bit, bull, facing, ball scallop | 33 | 0 |
//!
//!   **Every tapered-ball row the LUT ships prints a minimum below the crate's
//!   floor**, and they run the whole diameter range, not just the small end:
//!   Ø0.794 → 0.0191, Ø1.0 → 0.0191, Ø1.5 → 0.0127, Ø3.175 → 0.010,
//!   **Ø6.0 → 0.018**. Meanwhile the Ø0.794 and Ø1.5 *flat pocket* rows print
//!   0.0254–0.0762, comfortably above it. So "small tools" is the wrong axis on
//!   its own — a Ø6 tapered ball in hardwood is told by its own vendor row that
//!   0.018 mm/tooth is inside the window, and by this crate that 0.025 is the
//!   minimum. Any diameter-only law (P2 below) leaves that Ø6 case exactly where
//!   it is. This is why P1 is ranked first.
//! - Per-series log-log fits of `chipload_min` against the row's published
//!   diameter, wood rows only, grouped as `CREDITS.md` groups them
//!   (source x subfamily x tool family x material family x flutes x pass role):
//!   **median exponent 0.63 across 36 series**, row-weighted mean 0.67 over 113
//!   rows, and **0.586 / 0.619 on the two widest-span series** (Amana Spektra,
//!   16x diameter span). The vendors' own *lower* bound scales with diameter at
//!   very nearly the exponent the crate already uses for the band as a whole.
//! - The repo's own literature matrix **already** varies its floor by diameter,
//!   cell by cell, while the constant does not. 37 cells write a
//!   `chipload_above_rubbing_floor` invariant, across **eight distinct values**
//!   from 0.005 to 0.025: Ø1 ball 0.010, Ø1.5 flat 0.008, Ø2 tapered 0.012,
//!   Ø3 0.012–0.020, Ø6 and above 0.005–0.025. **19 of the 28 cells at Ø6 and
//!   above carry exactly 0.025** — the nine that do not are five V-bits (whose
//!   `diameter_mm` is a cone nominal), a tapered ball, a bull-nose scallop and two
//!   non-wood materials. **Not one of the three cells below Ø3 carries 0.025.**
//!   Log-log slope of the ladder with V-bits excluded: **0.44 over 32 cells** —
//!   flatter than the vendor `chipload_min` series (0.63) and than the crate's
//!   band law (0.61), and decisively not zero, which is what the code ships.
//!
//! ## The proposal, in preference order
//!
//! **P1 — hand the floor the band it already has, before inventing a law.** The
//! live Ø1 case is not primarily a missing-diameter-law case; it is a
//! **wrong-resolver** case. `chipload_bounds` comes from the *recipe* resolver
//! (`find_best_row_for_geometry`), which lets RPM-only anchors win. On that Ø1
//! tapered ball the winner is plausibly `whiteside-sc64-conical-ball-nose-…`
//! (Ø1.442, `chipload_min` absent), so `RequireBoth` yields `None` and
//! `effective_rubbing_floor` falls back to the bare constant — even though
//! `amana-tapered-hardwood-parallel-3175-2f` (0.010–0.020 as printed) is sitting
//! underneath, and is exactly what the post-sim gate resolves for the same cut.
//! The crate already detects this condition and names it
//! (`FeedsWarning::VendorRowPublishesNoChipload`) and already ships the envelope
//! resolver the gate uses. Subordinating the floor to *that* band when the recipe
//! resolver's row publishes none introduces **no new constant and no new
//! exponent**, and makes Suggest's floor and the gate's envelope quote the same
//! row — the "one decision consulted twice" rule the Checkpoint-K comments in
//! `feeds/mod.rs` already state. `report_a_band_the_floor_never_saw` measures how
//! much of the binding population P1 alone would resolve.
//!
//! ### P1 WAS IMPLEMENTED, AND THIS PROPOSAL'S EXAMPLE WAS WRONG — 2026-08-22
//!
//! Read this before citing the paragraph above. P1 shipped (`feeds/mod.rs`,
//! `floor_band_fallback`; sentries in `rubbing_floor_envelope_band_p1.rs`) and
//! the live case it was written for turned out **not** to be a wrong-resolver
//! case. The "plausibly" above was carrying real uncertainty, and probing the
//! exact input resolved it against the guess:
//!
//! ```text
//! Ø1.0 tapered ball (tip r0.5, 7°), 2F, white oak, ap 0.3, parallel/finish
//!   recipe resolver   -> amana-tapered-hardwood-parallel-3175-2f
//!   envelope resolver -> amana-tapered-hardwood-parallel-3175-2f    SAME ROW
//!   chipload_bounds    = Some(0.00484 .. 0.00968)
//!   floor applied      = 0.00968   (= min(0.025, band max), already correct)
//! ```
//!
//! `whiteside-sc64-conical-ball-nose-…` does not win; no RPM-only anchor is in
//! the way; `chipload_bounds` is `Some`; and the floor was **never** the bare
//! 0.025 on this cut. The subordination rule was already doing its job.
//!
//! The live symptom this file opens with — a ~0.012 request raised to a flat
//! 0.025 — reproduces on the same tool under `contour/finish`, where **neither**
//! resolver matches any row. That is a no-vendor-data case, not a
//! wrong-resolver one, and no resolver fix can reach it: with no row there is
//! no band to subordinate to. Only a floor carrying a diameter would, which is
//! **P2**, still not adopted. `band_capped_from` distinguishes the two shapes
//! on a live surface — `Some(0.025)` means a band was found and beat the
//! constant, `None` means the bare constant applied because nothing was found.
//!
//! Two consequences for anyone reading this file as evidence:
//!
//! 1. **The ranking argument stands; its example does not.** P1 is still the
//!    right first move (a floor consulting the resolver that cannot see bands
//!    is wrong regardless), and it is still true that any diameter-only law
//!    leaves the Ø6 tapered-ball conflict untouched. What is withdrawn is the
//!    claim that P1 reaches the observed Ø1 case.
//! 2. **P1 changes no recipe on the LUT as shipped.** Measured, not assumed:
//!    the cells where the two resolvers disagree are the Ø6-and-up flat/bull
//!    ones, whose envelope bands sit above 0.025, so `min` returns the constant
//!    unchanged. `the_fallback_does_not_lower_the_floor_on_todays_lut` is the
//!    tripwire that reports the day that stops being true.
//!
//! So the question this file exists to answer is **not** closed by P1. It is
//! sharper: the binding cases are the ones with no vendor row at all, and the
//! quantity governing them is edge radius — which, as "What no source backs"
//! below already says, nothing in this repo measures.
//!
//! **P2 — if a scaled floor is still wanted after P1, the form the evidence
//! supports is a down-scale anchored at Ø6 and capped at the current constant:**
//!
//! ```text
//! floor(D) = RUBBING_FLOOR_MM_TOOTH * min(1, (D_engaged / 6) ^ CHIPLOAD_DIAMETER_EXPONENT)
//! ```
//!
//! Its properties, and why each is an argument:
//!
//! - It introduces **no new number**. The exponent is
//!   `vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT` (0.61), already shipped and
//!   already the formula fallback's `p`. The floor stops being the one chipload
//!   quantity in the crate that carries no diameter.
//! - The `min(1, …)` cap means **nothing at or above Ø6 moves**: no operating
//!   point there sees a different floor, so no literature-matrix cell at Ø6+ can
//!   move either, and `_litmatrix_rubbing_floor_clamp`'s Ø6 Ipe cell — the pin on
//!   this constant — is untouched in both directions. A floor that only *lowers*,
//!   and only below the anchor, cannot make any recipe more aggressive than it is
//!   today, so the change is one-sided in the safe direction. The price of that
//!   safety is stated above: it also leaves the Ø6 tapered-ball conflict
//!   unresolved. P1 is the part of this proposal that reaches that case.
//! - Its predictions land inside the matrix's own ladder where the ladder is
//!   monotone: Ø2 → 0.0128 (matrix 0.012), Ø3 → 0.0164 (matrix 0.012–0.020),
//!   Ø6 → 0.025 (matrix 0.025). Where the ladder is **not** monotone it cannot
//!   fit: the matrix has Ø1 ball 0.010 above Ø1.5 flat 0.008, and P2 predicts
//!   0.0084 and 0.0107 — the right magnitude, the opposite order. Stated rather
//!   than smoothed: no smooth curve passes through both of those cells, so P2
//!   should be read as tracking a band, not reproducing a ladder.
//! - Anchoring at Ø6 rather than fitting an intercept is a **choice**, not a
//!   measurement: Ø6 is where the shipped LUT is densest (26 wood rows) and where
//!   19 of 28 matrix cells already agree on 0.025. It is not unanimous there, and
//!   the exponent is not settled either — the matrix's own ladder fits 0.44, not
//!   0.61. P2 takes 0.61 because it is already in the codebase and adopting a
//!   *third* exponent to fix a *first* inconsistency is worse than reusing one.
//!   Anyone who prefers 0.44 should say so against these same two fits, not
//!   against a new one.
//!
//! **P3 — rejected, recorded so it is not re-proposed.** "When no band exists,
//! derive the floor from `ChipLoadFormula` itself, e.g. as a fixed fraction of the
//! formula chipload." That makes the bound a multiple of the quantity it bounds:
//! the comparison can then never fail, whatever the material or the diameter does,
//! which is the vacuous-bar failure mode this repo has now hit four times. Do not
//! adopt it.
//!
//! ## What no source backs, said plainly
//!
//! **No primary source in this repo backs a diameter-scaled rubbing floor, and
//! none backs the 0.025 constant either.** `CREDITS.md` already says the `D^0.61`
//! and `Janka^-0.5` laws are repo-derived and that no primary source publishes a
//! chipload–diameter exponent at all; borrowing 0.61 for the floor inherits that
//! status exactly, and must be documented in the same words. The vendor evidence
//! above is about **the bottom of a recommended operating window**, which is not
//! the same measure as a **rubbing / minimum-chip-thickness threshold**. It is the
//! closest available proxy, not the quantity itself, and conflating the two is the
//! instrument-integrity error this repo has a rule about.
//!
//! The physically governing variable is not diameter at all: minimum chip
//! thickness is set by the **cutting-edge radius**, and diameter is only a proxy
//! for it because small cutters are ground with smaller edge radii. The crate
//! knows this — `ChiploadSource::EdgeRadiusFloor` is a declared variant that
//! **nothing ever constructs**. It is matched in five places, including a GUI
//! provenance chip that renders the label "edge-radius floor" and a `⌊` glyph in
//! `rs_cam_viz::ui::components::provenance`, and no code path in the workspace
//! assigns it to `FeedsResult::chipload_source`. It is a named, wired-up, empty
//! slot for the model that would actually answer this question. A bench
//! measurement of burn onset against advance per tooth on two or three tool sizes
//! would settle it; nothing short of that will.
//!
//! # Reproduce
//!
//! ```text
//! cargo test -p rs_cam_core --test rubbing_floor_diameter_scaling_measurement \
//!     -- --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::collections::{BTreeMap, BTreeSet};

use rs_cam_core::feeds::geometry::{ChiploadBoundPolicy, derate_chipload_bounds};
use rs_cam_core::feeds::vendor_lookup::find_best_chip_envelope_row;
use rs_cam_core::feeds::vendor_lut::MaterialFamily;
use rs_cam_core::feeds::vendor_normalize::to_lookup_query;
use rs_cam_core::feeds::{
    FeedsInput, FeedsWarning, OperationFamily, PassRole, RUBBING_FLOOR_MM_TOOTH, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, effective_rubbing_floor, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The anchor diameter P2 would scale from. Used **only** to print a model column
/// beside the matrix's own floors for comparison — it applies nothing.
const P2_ANCHOR_DIAMETER_MM: f64 = 6.0;

/// The exponent P2 would borrow, restated locally so this file's model column does
/// not silently track a production constant it is arguing about.
/// Mirrors `vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT`.
const P2_EXPONENT: f64 = 0.61;

// ---------------------------------------------------------------------------
// Sweep fixture
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Flat,
    Ball,
    TaperedBall,
}

impl Class {
    fn label(self) -> &'static str {
        match self {
            Class::Flat => "flat",
            Class::Ball => "ball",
            Class::TaperedBall => "tapered ball",
        }
    }
}

struct ToolSpec {
    label: &'static str,
    class: Class,
    /// Nominal published diameter. For a tapered ball this is the **tip**
    /// diameter, which is what an operator reads off the tool and what the
    /// "2.5 % of diameter" observation was taken against.
    diameter_mm: f64,
    flutes: u32,
    flute_length_mm: f64,
    shank_mm: f64,
    taper_deg: f64,
}

impl ToolSpec {
    fn geometry(&self) -> ToolGeometryHint {
        match self.class {
            Class::Flat => ToolGeometryHint::Flat,
            Class::Ball => ToolGeometryHint::Ball,
            Class::TaperedBall => ToolGeometryHint::TaperedBall {
                tip_radius: self.diameter_mm * 0.5,
                taper_angle_deg: self.taper_deg,
            },
        }
    }
}

/// A cut dimension expressed the way an operator sets it — as a fraction of
/// diameter for 2.5D work, as an absolute millimetre for 3D finishing, where a
/// 0.1 mm stepdown is 0.1 mm whatever tool is holding it.
#[derive(Debug, Clone, Copy)]
enum Dim {
    OfDiameter(f64),
    Absolute(f64),
}

impl Dim {
    fn mm(self, diameter_mm: f64) -> f64 {
        match self {
            Dim::OfDiameter(fraction) => fraction * diameter_mm,
            Dim::Absolute(mm) => mm,
        }
    }
}

struct CutCase {
    label: &'static str,
    family: OperationFamily,
    role: PassRole,
    ap: Dim,
    ae: Dim,
    /// Cutter classes this case is representative for. Scallop is ball-tip only
    /// (CLAUDE.md: "Scallop requires a ball-tip tool"); the roughing cases are not
    /// run on tapered balls because nobody roughs with one.
    applies_to: &'static [Class],
}

const ALL_CLASSES: &[Class] = &[Class::Flat, Class::Ball, Class::TaperedBall];
const ROUGHING_CLASSES: &[Class] = &[Class::Flat, Class::Ball];
const BALL_TIP_CLASSES: &[Class] = &[Class::Ball, Class::TaperedBall];

const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        label: "flat D0.5 2F",
        class: Class::Flat,
        diameter_mm: 0.5,
        flutes: 2,
        flute_length_mm: 1.5,
        shank_mm: 3.175,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "flat D1 2F",
        class: Class::Flat,
        diameter_mm: 1.0,
        flutes: 2,
        flute_length_mm: 3.0,
        shank_mm: 3.175,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "flat D2 2F",
        class: Class::Flat,
        diameter_mm: 2.0,
        flutes: 2,
        flute_length_mm: 8.0,
        shank_mm: 3.175,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "flat D3 2F",
        class: Class::Flat,
        diameter_mm: 3.0,
        flutes: 2,
        flute_length_mm: 12.0,
        shank_mm: 3.175,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "flat D6 2F",
        class: Class::Flat,
        diameter_mm: 6.0,
        flutes: 2,
        flute_length_mm: 22.0,
        shank_mm: 6.35,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "flat D6 4F",
        class: Class::Flat,
        diameter_mm: 6.0,
        flutes: 4,
        flute_length_mm: 22.0,
        shank_mm: 6.35,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "flat D12 3F",
        class: Class::Flat,
        diameter_mm: 12.0,
        flutes: 3,
        flute_length_mm: 38.0,
        shank_mm: 12.0,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "ball D1 2F",
        class: Class::Ball,
        diameter_mm: 1.0,
        flutes: 2,
        flute_length_mm: 3.0,
        shank_mm: 3.175,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "ball D3 2F",
        class: Class::Ball,
        diameter_mm: 3.0,
        flutes: 2,
        flute_length_mm: 12.0,
        shank_mm: 3.175,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "ball D6 2F",
        class: Class::Ball,
        diameter_mm: 6.0,
        flutes: 2,
        flute_length_mm: 22.0,
        shank_mm: 6.35,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "ball D12 2F",
        class: Class::Ball,
        diameter_mm: 12.0,
        flutes: 2,
        flute_length_mm: 38.0,
        shank_mm: 12.0,
        taper_deg: 0.0,
    },
    ToolSpec {
        label: "tball tip D0.5 2F",
        class: Class::TaperedBall,
        diameter_mm: 0.5,
        flutes: 2,
        flute_length_mm: 20.0,
        shank_mm: 3.175,
        taper_deg: 5.2,
    },
    ToolSpec {
        label: "tball tip D1 2F",
        class: Class::TaperedBall,
        diameter_mm: 1.0,
        flutes: 2,
        flute_length_mm: 25.0,
        shank_mm: 6.0,
        taper_deg: 5.2,
    },
    ToolSpec {
        label: "tball tip D2 2F",
        class: Class::TaperedBall,
        diameter_mm: 2.0,
        flutes: 2,
        flute_length_mm: 25.0,
        shank_mm: 6.0,
        taper_deg: 5.2,
    },
    ToolSpec {
        label: "tball tip D3 2F",
        class: Class::TaperedBall,
        diameter_mm: 3.0,
        flutes: 2,
        flute_length_mm: 25.0,
        shank_mm: 6.0,
        taper_deg: 5.2,
    },
    ToolSpec {
        label: "tball tip D6 2F",
        class: Class::TaperedBall,
        diameter_mm: 6.0,
        flutes: 2,
        flute_length_mm: 30.0,
        shank_mm: 6.35,
        taper_deg: 5.2,
    },
];

const CASES: &[CutCase] = &[
    CutCase {
        label: "slot rough",
        family: OperationFamily::Pocket,
        role: PassRole::Roughing,
        ap: Dim::OfDiameter(1.0),
        ae: Dim::OfDiameter(1.0),
        applies_to: ROUGHING_CLASSES,
    },
    CutCase {
        label: "pocket rough",
        family: OperationFamily::Pocket,
        role: PassRole::Roughing,
        ap: Dim::OfDiameter(0.5),
        ae: Dim::OfDiameter(0.4),
        applies_to: ROUGHING_CLASSES,
    },
    CutCase {
        label: "adaptive rough",
        family: OperationFamily::Adaptive,
        role: PassRole::Roughing,
        ap: Dim::OfDiameter(1.0),
        ae: Dim::OfDiameter(0.15),
        applies_to: ROUGHING_CLASSES,
    },
    CutCase {
        label: "contour finish",
        family: OperationFamily::Contour,
        role: PassRole::Finish,
        ap: Dim::OfDiameter(1.0),
        ae: Dim::OfDiameter(0.10),
        applies_to: ALL_CLASSES,
    },
    CutCase {
        label: "raster finish",
        family: OperationFamily::Parallel,
        role: PassRole::Finish,
        ap: Dim::Absolute(0.50),
        ae: Dim::OfDiameter(0.15),
        applies_to: ALL_CLASSES,
    },
    CutCase {
        label: "scallop finish",
        family: OperationFamily::Scallop,
        role: PassRole::Finish,
        ap: Dim::Absolute(0.25),
        ae: Dim::Absolute(0.10),
        applies_to: BALL_TIP_CLASSES,
    },
    CutCase {
        label: "fine 3D finish",
        family: OperationFamily::Scallop,
        role: PassRole::Finish,
        ap: Dim::Absolute(0.10),
        ae: Dim::Absolute(0.05),
        applies_to: BALL_TIP_CLASSES,
    },
];

const ALL_SPECIES: [WoodSpecies; 10] = [
    WoodSpecies::GenericSoftwood,
    WoodSpecies::RadiataPine,
    WoodSpecies::LongleafPine,
    WoodSpecies::GenericHardwood,
    WoodSpecies::HardMaple,
    WoodSpecies::Walnut,
    WoodSpecies::Birch,
    WoodSpecies::WhiteOak,
    WoodSpecies::Jarrah,
    WoodSpecies::Ipe,
];

/// One swept operating point, carrying every quantity this file reports on.
struct Row {
    tool: &'static str,
    class: Class,
    nominal_d_mm: f64,
    /// The **LUT-semantics** engaged diameter at the requested DOC — the same
    /// expression `feeds::calculate` and `vendor_normalize` use to decide which
    /// vendor row applies. For flat and ball cutters this is the nominal diameter.
    engaged_d_mm: f64,
    case: &'static str,
    species: WoodSpecies,
    machine: &'static str,
    banded: bool,
    /// `true` when the rubbing-floor clamp fired.
    bound: bool,
    /// Pre-clamp commanded advance per tooth. Meaningful only when `bound`.
    requested_fpt: f64,
    /// The floor actually applied — the global constant, or the matched band's
    /// derated ceiling when that sat lower. Meaningful only when `bound`.
    applied_floor: f64,
    /// `Some(global)` when the band capped the floor below the global constant.
    band_capped_from: Option<f64>,
    /// The advance per tooth the recipe finally commands.
    final_fpt: f64,
    /// `true` when the recipe rests on a vendor row that publishes no chipload
    /// column at all — the condition P1 addresses.
    rpm_only_row: bool,
    /// The band ceiling the **envelope** resolver would have supplied for the same
    /// query and the same DOC derate, when the recipe resolver supplied none.
    counterfactual_band_max: Option<f64>,
}

impl Row {
    /// How far the clamp asked to lift the advance.
    fn demanded_factor(&self) -> f64 {
        self.applied_floor / self.requested_fpt
    }

    /// How far it actually lifted it — lower than `demanded_factor` when the
    /// machine feed cap intervened.
    fn realised_factor(&self) -> f64 {
        self.final_fpt / self.requested_fpt
    }

    fn floor_fraction_of_nominal_d(&self) -> f64 {
        self.applied_floor / self.nominal_d_mm
    }

    fn floor_fraction_of_engaged_d(&self) -> f64 {
        self.applied_floor / self.engaged_d_mm
    }
}

/// The band ceiling the **envelope** resolver would supply for this input — the
/// resolver the post-sim gate uses, which skips rows publishing no chipload
/// column. Same query, same DOC derate, same both-bounds policy that
/// `feeds::calculate` applies when it builds `chipload_bounds`.
fn envelope_band_max(input: &FeedsInput<'_>, engaged_d: f64, ap: f64) -> Option<f64> {
    let lut = input.vendor_lut?;
    let query = to_lookup_query(input)?;
    let row = find_best_chip_envelope_row(lut, &query, &input.tool_geometry)?;
    let doc_ratio = if engaged_d > 0.0 { ap / engaged_d } else { 0.0 };
    let band = derate_chipload_bounds(
        row.chip_load_min_mm,
        row.chip_load_max_mm,
        doc_ratio,
        ChiploadBoundPolicy::RequireBoth,
    )?;
    Some(band.max_mm_per_tooth)
}

fn sweep() -> Vec<Row> {
    let lut = embedded_vendor_lut();
    let mut rows = Vec::new();

    for (preset_label, machine) in MachineProfile::presets() {
        for tool in TOOLS {
            let geometry = tool.geometry();
            for case in CASES {
                if !case.applies_to.contains(&tool.class) {
                    continue;
                }
                let ap = case.ap.mm(tool.diameter_mm);
                let ae = case.ae.mm(tool.diameter_mm);
                let engaged_d =
                    geometry.engaged_diameter_at_doc(ap, tool.diameter_mm, tool.shank_mm);

                for species in ALL_SPECIES {
                    let material = Material::SolidWood { species };
                    let input = FeedsInput {
                        tool_diameter: tool.diameter_mm,
                        flute_count: tool.flutes,
                        flute_length: tool.flute_length_mm,
                        shank_diameter: Some(tool.shank_mm),
                        tool_geometry: geometry,
                        material: &material,
                        machine: &machine,
                        operation: case.family,
                        operation_kind: None,
                        pass_role: case.role,
                        axial_depth_mm: Some(ap),
                        radial_width_mm: Some(ae),
                        target_scallop_mm: None,
                        vendor_lut: Some(lut),
                        setup: SetupContext::default(),
                        spindle_strategy: SpindleStrategy::MatchChart,
                    };
                    let result = calculate(&input);
                    let divisor = result.rpm * f64::from(tool.flutes);
                    assert!(
                        divisor > 0.0,
                        "{} / {} / {:?}: the engine produced rpm {} and flute count {} \
                         — every advance per tooth below would be a division by zero \
                         and the whole sweep vacuous",
                        tool.label,
                        case.label,
                        species,
                        result.rpm,
                        tool.flutes,
                    );
                    let final_fpt = result.feed_rate_mm_min / divisor;

                    let mut clamp = None;
                    let mut rpm_only_row = false;
                    for warning in &result.warnings {
                        match warning {
                            FeedsWarning::ChiploadClampedToFloor {
                                requested,
                                floor,
                                band_capped_from,
                            } => clamp = Some((*requested, *floor, *band_capped_from)),
                            FeedsWarning::VendorRowPublishesNoChipload { .. } => {
                                rpm_only_row = true;
                            }
                            _ => {}
                        }
                    }

                    // The counterfactual: same query, same DOC derate, but through
                    // the resolver that excludes chipload-less rows — i.e. the band
                    // the post-sim gate will judge this very cut against.
                    let counterfactual_band_max = if result.chipload_bounds.is_some() {
                        None
                    } else {
                        envelope_band_max(&input, engaged_d, ap)
                    };

                    rows.push(Row {
                        tool: tool.label,
                        class: tool.class,
                        nominal_d_mm: tool.diameter_mm,
                        engaged_d_mm: engaged_d,
                        case: case.label,
                        species,
                        machine: preset_label,
                        banded: result.chipload_bounds.is_some(),
                        bound: clamp.is_some(),
                        requested_fpt: clamp.map_or(final_fpt, |c| c.0),
                        applied_floor: clamp.map_or(f64::NAN, |c| c.1),
                        band_capped_from: clamp.and_then(|c| c.2),
                        final_fpt,
                        rpm_only_row,
                        counterfactual_band_max,
                    });
                }
            }
        }
    }
    rows
}

// ---------------------------------------------------------------------------
// Small statistics helpers — deliberately local, so this file states its own
// arithmetic rather than inheriting someone else's definition of a percentile.
// ---------------------------------------------------------------------------

fn ascending(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted
}

fn quantile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let sorted = ascending(values);
    let last = sorted.len() - 1;
    let idx = ((last as f64) * q).round() as usize;
    sorted[idx.min(last)]
}

fn pct(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return f64::NAN;
    }
    100.0 * (part as f64) / (whole as f64)
}

/// Ordinary-least-squares slope of `ln(y)` on `ln(x)`. `None` when there are
/// fewer than two points, when any coordinate is non-positive, or when every `x`
/// is the same — an exponent measured over a single diameter is not an exponent.
fn log_log_slope(points: &[(f64, f64)]) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let n = points.len() as f64;
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    for &(x, y) in points {
        if x.is_nan() || x <= 0.0 || y.is_nan() || y <= 0.0 {
            return None;
        }
        let lx = x.ln();
        let ly = y.ln();
        sx += lx;
        sy += ly;
        sxx += lx * lx;
        sxy += lx * ly;
    }
    let denom = n * sxx - sx * sx;
    if denom.abs() < 1e-12 {
        return None;
    }
    Some((n * sxy - sx * sy) / denom)
}

/// Group key for the per-diameter tables. Millimetres to three decimals, so
/// Ø3.175 and Ø3.0 stay distinct.
fn diameter_key(diameter_mm: f64) -> i64 {
    (diameter_mm * 1000.0).round() as i64
}

/// P2's predicted floor at this diameter. Printed for comparison ONLY: it is not
/// applied, not asserted against, and is not a threshold.
fn p2_model_floor(diameter_mm: f64) -> f64 {
    let ratio = diameter_mm / P2_ANCHOR_DIAMETER_MM;
    RUBBING_FLOOR_MM_TOOTH * ratio.powf(P2_EXPONENT).min(1.0)
}

// ---------------------------------------------------------------------------
// 1. How often does the floor bind, and by how much?
// ---------------------------------------------------------------------------

fn print_population_row(label: &str, population: &[&Row]) {
    let mut factors = Vec::new();
    for row in population {
        if row.bound {
            factors.push(row.demanded_factor());
        }
    }
    eprintln!(
        "{:<12} {:>7} {:>7} {:>7.1}% {:>9.2} {:>9.2} {:>9.2}",
        label,
        population.len(),
        factors.len(),
        pct(factors.len(), population.len()),
        quantile(&factors, 0.50),
        quantile(&factors, 0.90),
        quantile(&factors, 1.00),
    );
}

#[test]
fn report_how_often_the_rubbing_floor_binds() {
    let rows = sweep();
    let total = rows.len();

    let mut banded: Vec<&Row> = Vec::new();
    let mut unbanded: Vec<&Row> = Vec::new();
    let mut bound: Vec<&Row> = Vec::new();
    for row in &rows {
        if row.banded {
            banded.push(row);
        } else {
            unbanded.push(row);
        }
        if row.bound {
            bound.push(row);
        }
    }

    // NON-VACUITY, before any statistic is printed over these populations.
    assert!(
        total > 0,
        "the sweep produced no operating points at all — TOOLS x CASES x species x \
         presets collapsed to nothing and every number below would be an empty mean"
    );
    assert!(
        !banded.is_empty(),
        "VACUOUS: not one of {total} swept points matched a vendor row publishing a \
         chipload band, so the banded/unbanded split this measurement exists to \
         make has only one side. Widen CASES toward families the LUT actually \
         carries (pocket / contour / adaptive for flats, parallel / scallop for \
         ball tips) before reading anything below."
    );
    assert!(
        !unbanded.is_empty(),
        "VACUOUS: every one of {total} swept points matched a banded row, so the \
         no-band branch of `effective_rubbing_floor` — the branch the live D1 case \
         hit — is not exercised at all. Add a tool x family pairing the LUT has no \
         rows for (e.g. a flat end mill on OperationFamily::Parallel) before \
         reading anything below."
    );
    assert!(
        !bound.is_empty(),
        "VACUOUS: the rubbing-floor clamp did not fire once across {total} swept \
         operating points, so every factor, fraction and split below would be a \
         statistic over an empty set. This is exactly the empty-population failure \
         this repo has now hit four times: it is NOT evidence that the floor is \
         harmless. Widen the sweep toward small diameters, hard species and \
         shallow finishing depths until it fires, then re-read."
    );

    let mut bound_banded = 0usize;
    let mut bound_unbanded = 0usize;
    let mut capped_by_band = 0usize;
    let mut capped_by_machine = 0usize;
    let mut demanded: Vec<f64> = Vec::new();
    let mut realised: Vec<f64> = Vec::new();
    for row in &bound {
        if row.banded {
            bound_banded += 1;
        } else {
            bound_unbanded += 1;
        }
        if row.band_capped_from.is_some() {
            capped_by_band += 1;
        }
        if row.realised_factor() < row.demanded_factor() - 1e-9 {
            capped_by_machine += 1;
        }
        demanded.push(row.demanded_factor());
        realised.push(row.realised_factor());
    }

    eprintln!("\n-- rubbing floor: does it bind, and how hard? --------------------------");
    eprintln!(
        "  swept {total} operating points ({} tools x up-to-{} cases x {} species x \
         {} presets)",
        TOOLS.len(),
        CASES.len(),
        ALL_SPECIES.len(),
        MachineProfile::presets().len(),
    );
    eprintln!(
        "  banded {:5} ({:5.1} %)   unbanded {:5} ({:5.1} %)",
        banded.len(),
        pct(banded.len(), total),
        unbanded.len(),
        pct(unbanded.len(), total),
    );
    eprintln!(
        "  floor BINDS on {:5} of {total} ({:5.1} %)",
        bound.len(),
        pct(bound.len(), total),
    );
    eprintln!();
    eprintln!(
        "{:<12} {:>7} {:>7} {:>8} {:>9} {:>9} {:>9}",
        "population", "rows", "bound", "bind %", "xmedian", "xp90", "xmax"
    );
    print_population_row("banded", &banded);
    print_population_row("unbanded", &unbanded);
    eprintln!(
        "{:<12} {:>7} {:>7} {:>7.1}% {:>9.2} {:>9.2} {:>9.2}",
        "ALL",
        total,
        bound.len(),
        pct(bound.len(), total),
        quantile(&demanded, 0.50),
        quantile(&demanded, 0.90),
        quantile(&demanded, 1.00),
    );
    eprintln!();
    eprintln!(
        "  realised lift (after the machine feed cap): median x{:.2}, p90 x{:.2}, \
         max x{:.2}",
        quantile(&realised, 0.50),
        quantile(&realised, 0.90),
        quantile(&realised, 1.00),
    );
    eprintln!(
        "  of {} binding rows: {capped_by_band} had the floor capped DOWN to a band \
         ceiling (`band_capped_from`), {capped_by_machine} could not reach the \
         floor because the machine feed cap intervened",
        bound.len(),
    );

    // Per-diameter table. This is the shape the proposal argues from.
    let mut by_diameter: BTreeMap<i64, Vec<&Row>> = BTreeMap::new();
    for row in &rows {
        let key = diameter_key(row.nominal_d_mm);
        by_diameter.entry(key).or_default().push(row);
    }
    eprintln!();
    eprintln!(
        "{:>8} {:>7} {:>7} {:>8} {:>8} {:>9} {:>9}",
        "nominal", "rows", "bound", "bind %", "banded%", "xmedian", "xmax"
    );
    for (key, group) in &by_diameter {
        let mut factors = Vec::new();
        let mut banded_here = 0usize;
        for row in group {
            if row.bound {
                factors.push(row.demanded_factor());
            }
            if row.banded {
                banded_here += 1;
            }
        }
        eprintln!(
            "D{:>7.3} {:>7} {:>7} {:>7.1}% {:>7.1}% {:>9.2} {:>9.2}",
            (*key as f64) / 1000.0,
            group.len(),
            factors.len(),
            pct(factors.len(), group.len()),
            pct(banded_here, group.len()),
            quantile(&factors, 0.50),
            quantile(&factors, 1.00),
        );
    }

    // The ten hardest binds, named, so a reader can reproduce one by hand.
    let mut worst = bound.clone();
    worst.sort_by(|a, b| {
        let (x, y) = (a.demanded_factor(), b.demanded_factor());
        y.partial_cmp(&x).unwrap()
    });
    eprintln!();
    eprintln!("  hardest binds:");
    for row in worst.iter().take(10) {
        eprintln!(
            "    x{:5.2}  {:<18} {:<15} {:<13} {:<24} {:?}: requested {:.5} -> \
             floor {:.5} (band matched: {})",
            row.demanded_factor(),
            row.tool,
            row.case,
            row.class.label(),
            row.machine,
            row.species,
            row.requested_fpt,
            row.applied_floor,
            row.banded,
        );
    }
    eprintln!("------------------------------------------------------------------------\n");

    // Consistency of the warning itself, on every binding row. Not a new bar: a
    // clamp that reported `requested >= floor` would be describing something other
    // than the clamp it accompanies.
    for row in &bound {
        assert!(
            row.requested_fpt > 0.0 && row.requested_fpt < row.applied_floor,
            "{} / {} / {:?}: ChiploadClampedToFloor reported requested {:.6} \
             against applied floor {:.6} — a clamp warning must describe a genuine \
             lift",
            row.tool,
            row.case,
            row.species,
            row.requested_fpt,
            row.applied_floor,
        );
    }
    if bound_banded == 0 {
        eprintln!(
            "  NOTE: the floor bound ONLY on unbanded rows across this sweep. That \
             is the strongest form of the claim under test, not a vacuum — the \
             banded population is {} rows and was measured.",
            banded.len(),
        );
    }
    if bound_unbanded == 0 {
        eprintln!(
            "  NOTE: the floor bound ONLY on banded rows across this sweep, which \
             CONTRADICTS the claim that it mostly bites where there is no band. \
             The unbanded population is {} rows and was measured.",
            unbanded.len(),
        );
    }
}

// ---------------------------------------------------------------------------
// 2. The floor as a fraction of diameter
// ---------------------------------------------------------------------------

#[test]
fn report_the_bound_floor_as_a_fraction_of_tool_diameter() {
    let rows = sweep();
    let mut bound: Vec<&Row> = Vec::new();
    for row in &rows {
        if row.bound {
            bound.push(row);
        }
    }

    assert!(
        !bound.is_empty(),
        "VACUOUS: no swept point had the floor applied, so 'the floor as a \
         fraction of diameter' has nothing to average. See the guard in \
         `report_how_often_the_rubbing_floor_binds` for what to widen."
    );
    let mut distinct: BTreeSet<i64> = BTreeSet::new();
    for row in &bound {
        distinct.insert(diameter_key(row.nominal_d_mm));
    }
    assert!(
        distinct.len() >= 2,
        "VACUOUS: the floor was applied at only {} distinct nominal diameter(s). \
         The '2.5 % of a D1 tip vs 0.4 % of a D6' asymmetry is a statement ABOUT a \
         diameter span; measured at one diameter it is not measured at all.",
        distinct.len(),
    );

    let mut by_diameter: BTreeMap<i64, Vec<&Row>> = BTreeMap::new();
    for row in &bound {
        let key = diameter_key(row.nominal_d_mm);
        by_diameter.entry(key).or_default().push(*row);
    }

    eprintln!("\n-- the applied floor, as a fraction of the cutter ----------------------");
    eprintln!(
        "  the global constant is {RUBBING_FLOOR_MM_TOOTH} mm/tooth and carries no \
         diameter; these columns are what that means per tool"
    );
    eprintln!();
    eprintln!(
        "{:>8} {:>7} {:>11} {:>11} {:>11} {:>11}",
        "nominal", "bound", "floor mm", "% nominal", "% engaged", "% if 0.025"
    );
    for (key, group) in &by_diameter {
        let nominal = (*key as f64) / 1000.0;
        let mut floors = Vec::new();
        let mut frac_nominal = Vec::new();
        let mut frac_engaged = Vec::new();
        for row in group {
            floors.push(row.applied_floor);
            frac_nominal.push(row.floor_fraction_of_nominal_d());
            frac_engaged.push(row.floor_fraction_of_engaged_d());
        }
        eprintln!(
            "D{:>7.3} {:>7} {:>11.5} {:>10.2}% {:>10.2}% {:>10.2}%",
            nominal,
            group.len(),
            quantile(&floors, 0.50),
            100.0 * quantile(&frac_nominal, 0.50),
            100.0 * quantile(&frac_engaged, 0.50),
            100.0 * RUBBING_FLOOR_MM_TOOTH / nominal,
        );
    }

    let small_d = (*distinct.iter().next().unwrap() as f64) / 1000.0;
    let large_d = (*distinct.iter().next_back().unwrap() as f64) / 1000.0;
    eprintln!();
    eprintln!(
        "  span measured: D{small_d} -> {:.2} % of diameter, D{large_d} -> {:.2} % \
         of diameter, a {:.1}x asymmetry produced by ONE constant",
        100.0 * RUBBING_FLOOR_MM_TOOTH / small_d,
        100.0 * RUBBING_FLOOR_MM_TOOTH / large_d,
        large_d / small_d,
    );

    // What the pre-clamp model asked for, as an exponent. The engine's own no-band
    // chipload model is `K0 * D^0.61 * (1/H)^q`, so the requested advance should
    // itself carry a diameter law — which is the internal inconsistency the
    // proposal names: a D-scaled request overridden by a D-blind floor.
    let mut requests: Vec<(f64, f64)> = Vec::new();
    for row in &bound {
        if !row.banded {
            requests.push((row.nominal_d_mm, row.requested_fpt));
        }
    }
    match log_log_slope(&requests) {
        Some(slope) => eprintln!(
            "  pre-clamp requested advance vs nominal diameter, unbanded binds \
             (n = {}): d(ln fz)/d(ln D) = {slope:.3}",
            requests.len(),
        ),
        None => eprintln!(
            "  pre-clamp exponent NOT MEASURED: {} unbanded binding rows span too \
             few distinct diameters to fit one",
            requests.len(),
        ),
    }
    eprintln!("------------------------------------------------------------------------\n");
}

// ---------------------------------------------------------------------------
// 3. The counterfactual — a band the floor never saw
// ---------------------------------------------------------------------------

#[test]
fn report_a_band_the_floor_never_saw() {
    let rows = sweep();
    let mut unbanded: Vec<&Row> = Vec::new();
    for row in &rows {
        if !row.banded {
            unbanded.push(row);
        }
    }

    assert!(
        !unbanded.is_empty(),
        "VACUOUS: no swept point reached the no-band branch, so P1 — 'hand the \
         floor the envelope resolver's band' — has no population to be measured \
         against."
    );

    let mut with_envelope = 0usize;
    let mut rpm_only = 0usize;
    let mut bound_unbanded = 0usize;
    let mut resolvable = 0usize;
    let mut would_lower: Vec<&Row> = Vec::new();
    for row in &unbanded {
        if row.counterfactual_band_max.is_some() {
            with_envelope += 1;
        }
        if row.rpm_only_row {
            rpm_only += 1;
        }
        if !row.bound {
            continue;
        }
        bound_unbanded += 1;
        let Some(envelope_max) = row.counterfactual_band_max else {
            continue;
        };
        resolvable += 1;
        if envelope_max < row.applied_floor - 1e-12 {
            would_lower.push(*row);
        }
    }

    eprintln!("\n-- P1 counterfactual: the band the envelope resolver would have given --");
    eprintln!(
        "  unbanded rows: {}  (of which {rpm_only} rest on a vendor row that \
         publishes no chipload column — `VendorRowPublishesNoChipload`)",
        unbanded.len(),
    );
    eprintln!(
        "  an envelope row EXISTS for {with_envelope} of them ({:.1} %) — same \
         query, same DOC derate, resolver that skips chipload-less rows",
        pct(with_envelope, unbanded.len()),
    );
    eprintln!(
        "  of the {bound_unbanded} unbanded rows where the floor BOUND, \
         {resolvable} have an envelope band, and {} of those have a ceiling BELOW \
         the floor that was applied",
        would_lower.len(),
    );

    if would_lower.is_empty() {
        eprintln!(
            "  -> P1 resolves NONE of the measured binds. Either no envelope row \
             exists for these cuts, or its ceiling already clears the floor. On \
             this sweep P1 is not the lever; read P2 on its own evidence."
        );
    } else {
        let mut ratios = Vec::new();
        for row in &would_lower {
            ratios.push(row.applied_floor / row.counterfactual_band_max.unwrap());
        }
        eprintln!(
            "  -> on those {} rows the applied floor is x{:.2} (median) / x{:.2} \
             (max) the band ceiling the gate will judge the same cut against",
            would_lower.len(),
            quantile(&ratios, 0.50),
            quantile(&ratios, 1.00),
        );
        eprintln!("  worked examples:");
        would_lower.sort_by(|a, b| {
            let x = a.applied_floor / a.counterfactual_band_max.unwrap();
            let y = b.applied_floor / b.counterfactual_band_max.unwrap();
            y.partial_cmp(&x).unwrap()
        });
        for row in would_lower.iter().take(8) {
            let envelope_max = row.counterfactual_band_max.unwrap();
            eprintln!(
                "    {:<18} {:<15} {:?}: requested {:.5}, floor {:.5}, envelope max \
                 {:.5} -> x{:.2} over the gate's own ceiling",
                row.tool,
                row.case,
                row.species,
                row.requested_fpt,
                row.applied_floor,
                envelope_max,
                row.applied_floor / envelope_max,
            );
        }
    }
    eprintln!("------------------------------------------------------------------------\n");
}

// ---------------------------------------------------------------------------
// 4. What the vendors themselves publish as a minimum, versus diameter
// ---------------------------------------------------------------------------

fn is_wood(family: MaterialFamily) -> bool {
    matches!(
        family,
        MaterialFamily::Softwood
            | MaterialFamily::Hardwood
            | MaterialFamily::PlywoodSoftwood
            | MaterialFamily::PlywoodHardwood
            | MaterialFamily::Mdf
            | MaterialFamily::Hdf
            | MaterialFamily::Particleboard
    )
}

#[test]
fn report_vendor_published_chipload_minimum_versus_diameter() {
    let lut = embedded_vendor_lut();

    // As-published rows only: a diameter anchor and a printed chipload minimum. No
    // transfer law is applied here, and that is the point — this measures what the
    // vendors wrote, not what the crate does with it afterwards.
    let mut points: Vec<(f64, f64, &str)> = Vec::new();
    let mut groups: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    // (rows below the floor, rows total) per cutter family x vendor op family.
    // The 2026-08-22 reading of this table is what moved the proposal off a
    // pure-diameter framing: the rows the floor forbids are concentrated in the
    // 3D-profiling families at EVERY diameter, not in the small tools.
    let mut by_family: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for obs in &lut.observations {
        if !is_wood(obs.material_family) {
            continue;
        }
        let Some(diameter) = obs.diameter_mm else {
            continue;
        };
        let Some(min) = obs.chipload_min_mm_tooth else {
            continue;
        };
        if diameter.is_nan() || diameter <= 0.0 || min.is_nan() || min <= 0.0 {
            continue;
        }
        points.push((diameter, min, obs.observation_id.as_str()));
        let key = format!(
            "{} | {} | {:?} | {:?} | {}F | {:?} | {:?}",
            obs.source_id,
            obs.tool_subfamily.as_deref().unwrap_or("-"),
            obs.tool_family,
            obs.material_family,
            obs.flute_count,
            obs.pass_role,
            obs.operation_family,
        );
        groups.entry(key).or_default().push((diameter, min));
        let family = format!("{:?} / {:?}", obs.tool_family, obs.operation_family);
        let tally = by_family.entry(family).or_insert((0, 0));
        tally.1 += 1;
        if min < RUBBING_FLOOR_MM_TOOTH {
            tally.0 += 1;
        }
    }

    assert!(
        points.len() >= 20,
        "VACUOUS: only {} wood LUT rows publish both a diameter and a chipload \
         minimum. A diameter law fitted over that is not a measurement. Either the \
         LUT shrank or the wood-family filter in `is_wood` no longer matches the \
         shipped families.",
        points.len(),
    );

    let mut fits: Vec<(f64, usize, f64, String)> = Vec::new();
    for (key, series) in &groups {
        let Some(slope) = log_log_slope(series) else {
            continue;
        };
        let mut diameters = Vec::new();
        for point in series {
            diameters.push(point.0);
        }
        let lo = quantile(&diameters, 0.0);
        let hi = quantile(&diameters, 1.0);
        fits.push((slope, series.len(), hi / lo, key.to_owned()));
    }
    assert!(
        fits.len() >= 5,
        "VACUOUS: only {} vendor series span two or more diameters, so a 'median \
         exponent across series' would be a median of almost nothing. Do not quote \
         it.",
        fits.len(),
    );

    let mut below_floor: Vec<&(f64, f64, &str)> = Vec::new();
    for point in &points {
        if point.1 < RUBBING_FLOOR_MM_TOOTH {
            below_floor.push(point);
        }
    }
    let mut slopes = Vec::new();
    let mut weighted_numerator = 0.0;
    let mut weighted_rows = 0usize;
    for fit in &fits {
        slopes.push(fit.0);
        weighted_numerator += fit.0 * (fit.1 as f64);
        weighted_rows += fit.1;
    }
    let weighted = weighted_numerator / (weighted_rows as f64);

    eprintln!("\n-- what the vendors print as a MINIMUM, against diameter ---------------");
    eprintln!(
        "  wood rows with a diameter anchor and a printed chipload minimum: {}",
        points.len(),
    );
    eprintln!(
        "  of those, {} print a minimum BELOW the {RUBBING_FLOOR_MM_TOOTH} \
         mm/tooth floor — as printed, before any diameter or hardness transfer:",
        below_floor.len(),
    );
    below_floor.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for point in &below_floor {
        eprintln!(
            "    D{:>7.3}  min {:.4} mm/tooth  ({:.2} % of D)  {}",
            point.0,
            point.1,
            100.0 * point.1 / point.0,
            point.2,
        );
    }
    eprintln!();
    eprintln!(
        "  the same 19, split by cutter family — the axis that turned out to \
         matter as much as diameter:"
    );
    eprintln!(
        "{:<34} {:>7} {:>7} {:>9}",
        "cutter family / vendor op", "rows", "below", "below %"
    );
    for (family, tally) in &by_family {
        eprintln!(
            "{family:<34} {:>7} {:>7} {:>8.1}%",
            tally.1,
            tally.0,
            pct(tally.0, tally.1),
        );
    }
    eprintln!();
    eprintln!(
        "  per-series log-log fits of chipload_min vs published diameter ({} \
         series):",
        fits.len(),
    );
    fits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for fit in &fits {
        eprintln!(
            "    {:+.3}  n={:2} span={:5.2}x  {}",
            fit.0, fit.1, fit.2, fit.3
        );
    }
    eprintln!();
    eprintln!(
        "  median exponent across series {:.3}; row-weighted mean {weighted:.3} \
         over {weighted_rows} rows; the crate's own CHIPLOAD_DIAMETER_EXPONENT is \
         {P2_EXPONENT}",
        quantile(&slopes, 0.50),
    );
    eprintln!(
        "  READ THIS AS A PROXY, NOT THE QUANTITY. A vendor's chipload minimum is \
         the bottom of a recommended operating window; a rubbing floor is a \
         chip-formation threshold. They share a unit (mm advance per tooth) and \
         nothing else. No primary source in this repo publishes a diameter-scaled \
         rubbing floor."
    );
    eprintln!("------------------------------------------------------------------------\n");
}

// ---------------------------------------------------------------------------
// 5. The literature matrix already scales its own floor — the code does not
// ---------------------------------------------------------------------------

fn toml_number(value: &toml::Value) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|i| i as f64))
}

/// `(cell id, tool class, nominal diameter, per-cell rubbing floor)` for every
/// literature-matrix cell that writes a `chipload_above_rubbing_floor` invariant.
fn matrix_floor_cells() -> Vec<(String, String, f64, f64)> {
    let source = include_str!("literature_matrix/cells.toml");
    let doc: toml::Value = toml::from_str(source).expect("cells.toml parses");
    let mut out = Vec::new();
    let Some(cells) = doc.get("cell").and_then(toml::Value::as_array) else {
        return out;
    };
    for cell in cells {
        let Some(id) = cell.get("id").and_then(toml::Value::as_str) else {
            continue;
        };
        let inputs = cell.get("inputs");
        let class = inputs
            .and_then(|i| i.get("tool_class"))
            .and_then(toml::Value::as_str)
            .unwrap_or("?");
        let diameter = inputs
            .and_then(|i| i.get("diameter_mm"))
            .and_then(toml_number);
        let Some(diameter) = diameter else {
            continue;
        };
        let Some(invariants) = cell.get("invariants").and_then(toml::Value::as_array) else {
            continue;
        };
        for invariant in invariants {
            let name = invariant.get("name").and_then(toml::Value::as_str);
            if name != Some("chipload_above_rubbing_floor") {
                continue;
            }
            if let Some(floor) = invariant.get("floor").and_then(toml_number) {
                out.push((id.to_owned(), class.to_owned(), diameter, floor));
            }
        }
    }
    out
}

#[test]
fn report_the_literature_matrix_already_varies_its_own_floor() {
    let mut cells = matrix_floor_cells();

    assert!(
        cells.len() >= 10,
        "VACUOUS: found {} literature-matrix cells carrying a \
         `chipload_above_rubbing_floor` invariant. Either cells.toml moved, the \
         invariant was renamed, or the parse above is wrong — do not read the \
         ladder below as evidence of anything.",
        cells.len(),
    );
    let mut distinct: BTreeSet<i64> = BTreeSet::new();
    for cell in &cells {
        distinct.insert((cell.3 * 100_000.0).round() as i64);
    }
    assert!(
        distinct.len() >= 2,
        "VACUOUS: every matrix cell writes the SAME floor, so 'the matrix already \
         varies its floor by diameter' is not measured here. If that ever becomes \
         true it is a real finding, and this file's proposal must be re-argued \
         without it."
    );

    cells.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap().then(a.0.cmp(&b.0)));

    eprintln!("\n-- the matrix's own per-cell floors, against diameter ------------------");
    eprintln!(
        "  {} cells, {} distinct floor values, while the code ships ONE constant \
         ({RUBBING_FLOOR_MM_TOOTH})",
        cells.len(),
        distinct.len(),
    );
    eprintln!();
    eprintln!(
        "{:>8} {:<14} {:>9} {:>10} {:>10}  cell",
        "diameter", "class", "floor", "% of D", "P2 model"
    );
    for (id, class, diameter, floor) in &cells {
        eprintln!(
            "D{:>7.3} {:<14} {:>9.4} {:>9.2}% {:>10.4}  {id}",
            diameter,
            class,
            floor,
            100.0 * floor / diameter,
            p2_model_floor(*diameter),
        );
    }

    // Grouped, so the ladder is visible without reading every row.
    let mut by_diameter: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
    for cell in &cells {
        let key = diameter_key(cell.2);
        by_diameter.entry(key).or_default().push(cell.3);
    }
    eprintln!();
    eprintln!(
        "{:>8} {:>7} {:>10} {:>10} {:>12}",
        "diameter", "cells", "floor min", "floor max", "P2 model"
    );
    for (key, floors) in &by_diameter {
        let diameter = (*key as f64) / 1000.0;
        eprintln!(
            "D{:>7.3} {:>7} {:>10.4} {:>10.4} {:>12.4}",
            diameter,
            floors.len(),
            quantile(floors, 0.0),
            quantile(floors, 1.0),
            p2_model_floor(diameter),
        );
    }

    // V-bits are excluded from the fit on purpose: their `diameter_mm` is a
    // nominal cone diameter and their engaged width is set by depth, so a
    // floor-per-diameter reading of a V-bit cell is not the same measurement as
    // one of an end mill.
    let mut ladder: Vec<(f64, f64)> = Vec::new();
    for cell in &cells {
        if cell.1 != "vbit" {
            ladder.push((cell.2, cell.3));
        }
    }
    match log_log_slope(&ladder) {
        Some(slope) => eprintln!(
            "\n  log-log slope of the matrix's own floors vs diameter, V-bits \
             excluded (n = {}): {slope:.3}. The crate's CHIPLOAD_DIAMETER_EXPONENT \
             is {P2_EXPONENT}, and the code's floor is flat at 0.",
            ladder.len(),
        ),
        None => eprintln!(
            "\n  ladder slope NOT MEASURED: {} non-V-bit cells span too few \
             distinct diameters to fit one",
            ladder.len(),
        ),
    }
    eprintln!("------------------------------------------------------------------------\n");
}

// ---------------------------------------------------------------------------
// 6. The promise this file makes
// ---------------------------------------------------------------------------

#[test]
fn the_constant_this_instrument_measured_has_not_moved() {
    // This wave was authorised to MEASURE and to PROPOSE, not to recalibrate. The
    // assertion exists so a reader can tell, without archaeology, whether the
    // numbers this file prints were taken against the floor the crate still ships.
    //
    // If it ever fails that is NOT necessarily a defect: it means someone adopted a
    // new floor. Re-run the reports above before quoting any figure from this
    // file's docstring, because every one of them was measured at 0.025.
    assert!(
        (RUBBING_FLOOR_MM_TOOTH - 0.025).abs() < 1e-12,
        "RUBBING_FLOOR_MM_TOOTH is now {RUBBING_FLOOR_MM_TOOTH}, not the 0.025 \
         this instrument's measurements and proposal were taken against. Re-run \
         the reports in this file and re-read its docstring before quoting it."
    );
    // And the no-band branch still returns the bare constant — the branch the live
    // D1 tapered-ball case hit, and the one P1 and P2 both address.
    assert!(
        (effective_rubbing_floor(None) - RUBBING_FLOOR_MM_TOOTH).abs() < 1e-12,
        "the no-band branch of `effective_rubbing_floor` no longer returns the \
         bare constant. That is the branch this file measures; its findings need \
         re-taking."
    );
}
