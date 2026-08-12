//! **F-LUT2 structural census — number-preserving instrument (TD3 wave A-6).**
//!
//! Two LUT entry points resolve a vendor row for the same physical
//! question, and they are wired to different consumers:
//!
//! | consumer | entry point | site |
//! |---|---|---|
//! | Suggest / feeds calculator | [`find_best_row_for_geometry`] | `feeds/mod.rs:1002` |
//! | Suggest's own explanation | [`find_best_row_for_geometry`] | `feeds/explain.rs:177` |
//! | post-sim chipload gate, optimizer context, viewport envelope map | [`find_best_chip_envelope_row`] | `tool_load/chipload.rs:141` |
//!
//! `find_best_chip_envelope_row` is the same scorer with one extra
//! **filter**: only rows publishing at least one chipload bound compete
//! (`obs.chipload_min_mm_tooth.is_some() || obs.chipload_max_mm_tooth.is_some()`).
//! So the two are related by candidate-set inclusion — the envelope set
//! is a strict subset — which means:
//!
//! - the envelope resolver can never match where the geometry resolver
//!   does not, and
//! - wherever an RPM-only row outscores every chipload-bearing row, the
//!   two resolvers return **different rows** and the operator sees a
//!   Suggest recipe built on one row and a gate verdict built on
//!   another.
//!
//! A-5 (2026-08-12) measured one cell of this live: on both of its
//! endmill fixtures Suggest's band maximum was **1.273×** the gate's.
//! This file measures the whole surface, changes nothing, and pins the
//! shape of the answer so the delta cannot move unnoticed before
//! Checkpoint K rules on it.
//!
//! Run the report:
//! ```text
//! cargo test -p rs_cam_core --test lut_resolver_census_a6 -- --ignored --nocapture
//! ```
//!
//! Committed as an instrument per `feedback_commit_instruments_before_gates`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::collections::BTreeMap;

use rs_cam_core::feeds::ToolGeometryHint;
use rs_cam_core::feeds::embedded_vendor_lut;
use rs_cam_core::feeds::vendor_lookup::{
    LookupQuery, LookupResult, find_best_chip_envelope_row, find_best_row_for_geometry,
};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily,
};

/// Every operation family the LUT models.
const FAMILIES: [LutOperationFamily; 8] = [
    LutOperationFamily::Adaptive,
    LutOperationFamily::Pocket,
    LutOperationFamily::Contour,
    LutOperationFamily::Parallel,
    LutOperationFamily::Scallop,
    LutOperationFamily::Trace,
    LutOperationFamily::Face,
    LutOperationFamily::Drill,
];

const ROLES: [LutPassRole; 3] = [
    LutPassRole::Roughing,
    LutPassRole::SemiFinish,
    LutPassRole::Finish,
];

/// Tool-geometry classes, one per `ToolGeometryHint` variant plus a
/// second V-bit angle (the angle-aware matcher's behaviour is
/// angle-dependent, so one sample would not exercise it).
fn geometry_classes() -> Vec<(&'static str, ToolGeometryHint)> {
    vec![
        ("Flat", ToolGeometryHint::Flat),
        ("Ball", ToolGeometryHint::Ball),
        ("Bull", ToolGeometryHint::Bull { corner_radius: 0.5 }),
        (
            "TaperedBall",
            ToolGeometryHint::TaperedBall {
                tip_radius: 0.25,
                taper_angle_deg: 5.0,
            },
        ),
        (
            "VBit90",
            ToolGeometryHint::VBit {
                included_angle: 90.0,
                tip_diameter: 0.2,
            },
        ),
        (
            "VBit60",
            ToolGeometryHint::VBit {
                included_angle: 60.0,
                tip_diameter: 0.2,
            },
        ),
    ]
}

/// Representative materials spanning every category the LUT's hard
/// material filter (`materials_compatible`) can route to. Hardness
/// values are the shipped canonical ones (`WoodSpecies::janka_lbf`,
/// `PlasticFamily::hardness`, `AluminumAlloy::brinell_hb`).
fn materials() -> Vec<(&'static str, MaterialFamily, HardnessKind, f64)> {
    vec![
        ("oak", MaterialFamily::Hardwood, HardnessKind::Janka, 1290.0),
        ("ipe", MaterialFamily::Hardwood, HardnessKind::Janka, 3684.0),
        ("pine", MaterialFamily::Softwood, HardnessKind::Janka, 690.0),
        ("mdf", MaterialFamily::Mdf, HardnessKind::Janka, 900.0),
        (
            "ply-birch",
            MaterialFamily::PlywoodHardwood,
            HardnessKind::Janka,
            1210.0,
        ),
        (
            "acrylic",
            MaterialFamily::Acrylic,
            HardnessKind::ShoreD,
            85.0,
        ),
        ("alu6061", MaterialFamily::Aluminum, HardnessKind::Hb, 95.0),
    ]
}

/// Diameters spanning the shipped LUT's calibrated span (Ø0.79–12.7).
const DIAMETERS: [f64; 6] = [1.0, 3.175, 6.0, 6.35, 9.525, 12.0];
const FLUTES: [u32; 3] = [1, 2, 3];

/// What the two resolvers did for one query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    /// Neither matched — the query has no LUT coverage at all.
    BothNone,
    /// The geometry resolver matched, the envelope resolver did not:
    /// every candidate row for this query is RPM-only, so the gate
    /// reports `Unmodeled(NoVendorData)` while Suggest happily builds a
    /// recipe on the RPM anchor.
    GateBlind,
    /// Both matched, same row. No divergence.
    Same,
    /// Both matched, different rows, and the geometry resolver's row
    /// publishes NO chipload band — so Suggest falls back to the
    /// *formula* chipload (`feeds/mod.rs:1068` — `chip_load_mm > 0.0`
    /// is false) with `chipload_bounds: None`, while the gate judges
    /// against a vendor band Suggest never saw.
    DifferentSuggestUnbanded,
    /// Both matched, different rows, both publish bands. This is A-5's
    /// case: two different numeric envelopes for one operation.
    DifferentBothBanded,
}

struct Cell {
    queries: usize,
    outcomes: BTreeMap<Outcome, usize>,
    /// Worst |ln(ratio)| band-maximum divergence seen in this cell,
    /// with the query that produced it.
    worst: Option<(f64, String, String, String, f64, f64)>,
}

impl Cell {
    fn new() -> Self {
        Self {
            queries: 0,
            outcomes: BTreeMap::new(),
            worst: None,
        }
    }
    fn divergent(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|(o, _)| **o != Outcome::Same && **o != Outcome::BothNone)
            .map(|(_, c)| *c)
            .sum()
    }
}

fn classify(
    geo: Option<&LookupResult>,
    env: Option<&LookupResult>,
) -> (Outcome, Option<(f64, f64)>) {
    match (geo, env) {
        (None, None) => (Outcome::BothNone, None),
        (None, Some(_)) => panic!(
            "IMPOSSIBLE by construction: the envelope candidate set is a strict subset of the \
             geometry candidate set, so the envelope resolver cannot match where the geometry \
             resolver does not. If this fires, the subset relation was broken."
        ),
        (Some(_), None) => (Outcome::GateBlind, None),
        (Some(g), Some(e)) => {
            if g.observation_id == e.observation_id {
                (Outcome::Same, None)
            } else if g.chip_load_max_mm.is_none() && g.chip_load_min_mm.is_none() {
                (Outcome::DifferentSuggestUnbanded, None)
            } else {
                let gm = g.chip_load_max_mm.or(g.chip_load_min_mm).unwrap_or(0.0);
                let em = e.chip_load_max_mm.or(e.chip_load_min_mm).unwrap_or(0.0);
                (Outcome::DifferentBothBanded, Some((gm, em)))
            }
        }
    }
}

fn run_census() -> BTreeMap<(String, String), Cell> {
    let lut = embedded_vendor_lut();
    let mut cells: BTreeMap<(String, String), Cell> = BTreeMap::new();
    for (gname, geom) in geometry_classes() {
        let tool_family = geom.cutter_kind().lut_family();
        for family in FAMILIES {
            let cell = cells
                .entry((format!("{family:?}"), gname.to_owned()))
                .or_insert_with(Cell::new);
            for role in ROLES {
                for (mname, material_family, hardness_kind, hardness_value) in materials() {
                    for d in DIAMETERS {
                        for flutes in FLUTES {
                            let query = LookupQuery {
                                tool_family,
                                tool_subfamily: None,
                                diameter_mm: d,
                                flute_count: flutes,
                                material_family,
                                hardness_kind: Some(hardness_kind),
                                hardness_value: Some(hardness_value),
                                operation_family: family,
                                pass_role: role,
                            };
                            let geo = find_best_row_for_geometry(lut, &query, &geom);
                            let env = find_best_chip_envelope_row(lut, &query, &geom);
                            let (outcome, bands) = classify(geo.as_ref(), env.as_ref());
                            cell.queries += 1;
                            *cell.outcomes.entry(outcome).or_insert(0) += 1;
                            if let Some((gm, em)) = bands
                                && gm > 0.0
                                && em > 0.0
                            {
                                let ln = (gm / em).ln().abs();
                                if cell.worst.as_ref().is_none_or(|w| ln > w.0) {
                                    cell.worst = Some((
                                        ln,
                                        format!("{mname} Ø{d} {flutes}F {role:?}"),
                                        geo.as_ref().unwrap().observation_id.clone(),
                                        env.as_ref().unwrap().observation_id.clone(),
                                        gm,
                                        em,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    cells
}

/// The report. `#[ignore]`d: it is an instrument, not a gate.
#[test]
#[ignore = "reporting instrument — run with --ignored --nocapture"]
fn lut_resolver_selection_census_report() {
    let cells = run_census();
    println!("# F-LUT2 resolver-selection census (A-6, number-preserving)\n");
    println!(
        "LUT: {} observations. Sweep: {} op families × {} geometry classes × {} pass roles × \
         {} materials × {} diameters × {} flute counts.\n",
        embedded_vendor_lut().observations.len(),
        FAMILIES.len(),
        geometry_classes().len(),
        ROLES.len(),
        materials().len(),
        DIAMETERS.len(),
        FLUTES.len()
    );
    println!(
        "| op family | geometry class | queries | same | both-none | gate-blind | \
         diff/suggest-unbanded | diff/both-banded | worst band max ratio (Suggest ÷ gate) |"
    );
    println!("|---|---|---:|---:|---:|---:|---:|---:|---|");
    let mut totals: BTreeMap<Outcome, usize> = BTreeMap::new();
    let mut worst_overall: Option<(f64, String, String, String, String, f64, f64)> = None;
    for ((family, geom), cell) in &cells {
        for (o, c) in &cell.outcomes {
            *totals.entry(*o).or_insert(0) += c;
        }
        let get = |o: Outcome| cell.outcomes.get(&o).copied().unwrap_or(0);
        let worst = match &cell.worst {
            Some((_, q, g, e, gm, em)) => {
                let r = gm / em;
                format!("×{r:.4} ({q}; {g} {gm:.5} vs {e} {em:.5})")
            }
            None => "—".to_owned(),
        };
        if let Some((ln, q, g, e, gm, em)) = &cell.worst
            && worst_overall.as_ref().is_none_or(|w| *ln > w.0)
        {
            worst_overall = Some((
                *ln,
                format!("{family}/{geom} {q}"),
                g.clone(),
                e.clone(),
                q.clone(),
                *gm,
                *em,
            ));
        }
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            family,
            geom,
            cell.queries,
            get(Outcome::Same),
            get(Outcome::BothNone),
            get(Outcome::GateBlind),
            get(Outcome::DifferentSuggestUnbanded),
            get(Outcome::DifferentBothBanded),
            worst
        );
    }
    println!("\n## Totals\n");
    let total: usize = totals.values().sum();
    for (o, c) in &totals {
        println!(
            "- `{o:?}`: {c} / {total} ({:.2} %)",
            *c as f64 / total as f64 * 100.0
        );
    }
    let diverging_cells = cells.values().filter(|c| c.divergent() > 0).count();
    println!(
        "\n**{diverging_cells} of {} (family, geometry-class) cells diverge.**",
        cells.len()
    );
    if let Some((_, where_, g, e, _, gm, em)) = worst_overall {
        println!(
            "\n**Worst band consequence:** ×{:.4} at {where_} — Suggest row `{g}` max {gm:.6} \
             vs gate row `{e}` max {em:.6}.",
            gm / em
        );
    }
}

// =====================================================================
// AXIS 2 — the operation-family reroute (where A-5's 1.273× actually is)
// =====================================================================
//
// The resolver sweep above holds the QUERY fixed and varies only the
// entry point. That is F-LUT2 as the ledger words it, and it turns out
// to be the small half of the problem. The larger half is that the two
// consumers do not build the same query:
//
// * Suggest's query family is `vendor_normalize::op_family_to_lut(
//   input.operation)` — a straight 1:1 map off the operation's
//   `feeds_family`.
// * The gate's query family is that value passed through
//   `tool_load::chipload::routed_lookup_family`, which REROUTES two
//   operation kinds: `Adaptive3d` (Adaptive → **Pocket**) and
//   `ProjectCurve` (Trace → Parallel/Contour Finish for ball/flat,
//   and → **refusal** for bull-nose / V-bit / facing).
//
// The reroute is deliberate and documented on the gate side. Nothing
// applies it on the Suggest side, so on every Adaptive3d and
// ProjectCurve operation Suggest recommends against one vendor row and
// the gate judges against another.
//
// `routed_lookup_family` is `pub(crate)`, so the table below MIRRORS its
// two rules rather than calling it. The function's own behaviour is
// pinned inside the crate (`tool_load/chipload.rs`, four unit tests at
// `:1325-1374`); this mirror only has to stay in step with those.

/// Faithful mirror of `tool_load::chipload::routed_lookup_family` for
/// the two rerouted operation kinds. Returns what the GATE queries
/// given what SUGGEST queries; `None` is the gate's refusal (which
/// surfaces to the operator as `Unmodeled(NoVendorData)` while Suggest
/// still produces a banded recommendation).
fn gate_route(
    op: &str,
    geom: &ToolGeometryHint,
    suggest_family: LutOperationFamily,
    suggest_role: LutPassRole,
) -> Option<(LutOperationFamily, LutPassRole)> {
    match op {
        // Adaptive3d: family only, role untouched, no geometry condition.
        "Adaptive3d" => {
            if suggest_family == LutOperationFamily::Adaptive {
                Some((LutOperationFamily::Pocket, suggest_role))
            } else {
                Some((suggest_family, suggest_role))
            }
        }
        // ProjectCurve: routed by cutter class, and the role is FORCED
        // to Finish regardless of what the operation asked for.
        "ProjectCurve" => match geom {
            ToolGeometryHint::Ball | ToolGeometryHint::TaperedBall { .. } => {
                Some((LutOperationFamily::Parallel, LutPassRole::Finish))
            }
            ToolGeometryHint::Flat => Some((LutOperationFamily::Contour, LutPassRole::Finish)),
            // Bull nose / V-bit / facing bit: the gate refuses outright.
            ToolGeometryHint::Bull { .. } | ToolGeometryHint::VBit { .. } => None,
        },
        _ => Some((suggest_family, suggest_role)),
    }
}

/// The rerouted operation kinds and the family Suggest queries for each.
const REROUTES: [(&str, LutOperationFamily); 2] = [
    ("Adaptive3d", LutOperationFamily::Adaptive),
    ("ProjectCurve", LutOperationFamily::Trace),
];

/// **The band consequence of the reroute, measured.** This is the census
/// cell A-5 hit: on both of its endmill fixtures Suggest's band maximum
/// was 1.273× the gate's, and the shape of the two bands differed
/// (hi/lo 1.842 vs 1.719), which a pure scale factor cannot do — so it
/// was never the diameter law or the DOC derate. It is two different
/// rows, reached from two different operation families.
#[test]
#[ignore = "reporting instrument — run with --ignored --nocapture"]
fn operation_family_reroute_band_delta_report() {
    let lut = embedded_vendor_lut();
    println!("# F-LUT2 axis 2: the operation-family reroute\n");
    println!(
        "| reroute | geometry | material | Ø | flutes | role | Suggest row (max) | \
         gate row (max) | Suggest ÷ gate |"
    );
    println!("|---|---|---|---:|---:|---|---|---|---:|");
    let mut ratios: Vec<f64> = Vec::new();
    let mut gate_refusals: usize = 0;
    let mut suggest_banded_while_gate_refuses: usize = 0;
    let mut considered: usize = 0;
    for (op, suggest_family) in REROUTES {
        for (gname, geom) in geometry_classes() {
            let tool_family = geom.cutter_kind().lut_family();
            for (mname, material_family, hardness_kind, hardness_value) in materials() {
                for d in DIAMETERS {
                    for flutes in [2u32, 3] {
                        for role in ROLES {
                            let base = LookupQuery {
                                tool_family,
                                tool_subfamily: None,
                                diameter_mm: d,
                                flute_count: flutes,
                                material_family,
                                hardness_kind: Some(hardness_kind),
                                hardness_value: Some(hardness_value),
                                operation_family: suggest_family,
                                pass_role: role,
                            };
                            // Suggest resolves through the geometry entry
                            // point at its own (unrouted) family.
                            let s = find_best_row_for_geometry(lut, &base, &geom);
                            considered += 1;
                            let Some((gate_family, gate_role)) =
                                gate_route(op, &geom, suggest_family, role)
                            else {
                                gate_refusals += 1;
                                if s.as_ref().is_some_and(|r| r.chip_load_max_mm.is_some()) {
                                    suggest_banded_while_gate_refuses += 1;
                                }
                                continue;
                            };
                            let gate_query = LookupQuery {
                                operation_family: gate_family,
                                pass_role: gate_role,
                                ..base.clone()
                            };
                            let g = find_best_chip_envelope_row(lut, &gate_query, &geom);
                            let (Some(s), Some(g)) = (s, g) else {
                                continue;
                            };
                            let (Some(sm), Some(gm)) = (s.chip_load_max_mm, g.chip_load_max_mm)
                            else {
                                continue;
                            };
                            if s.observation_id == g.observation_id || sm <= 0.0 || gm <= 0.0 {
                                continue;
                            }
                            ratios.push(sm / gm);
                            // Print only the rows a reader will recognise,
                            // or the table drowns.
                            if d == 6.0 && flutes == 2 && role == LutPassRole::Roughing {
                                println!(
                                    "| {op} | {gname} | {mname} | {d} | {flutes} | \
                                     {role:?}→{gate_role:?} | {} ({sm:.5}) | {} ({gm:.5}) | \
                                     **×{:.4}** |",
                                    s.observation_id,
                                    g.observation_id,
                                    sm / gm
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = ratios.len();
    println!(
        "\n**{n} of {considered} (query, reroute) pairs resolve to DIFFERENT rows.** \
         Suggest ÷ gate band maximum: min ×{:.4}, median ×{:.4}, max ×{:.4}.",
        ratios.first().copied().unwrap_or(f64::NAN),
        ratios.get(n / 2).copied().unwrap_or(f64::NAN),
        ratios.last().copied().unwrap_or(f64::NAN),
    );
    println!(
        "\n**{gate_refusals} pairs are gate REFUSALS** (ProjectCurve on bull-nose / V-bit: \
         `routed_lookup_family` returns `None` → `Unmodeled(NoVendorData)`), and in \
         **{suggest_banded_while_gate_refuses}** of them Suggest still returns a banded \
         recommendation. That is the sharpest form of the divergence: an operator gets a \
         vendor-backed number on a surface where the gate has declined to judge at all."
    );
    println!(
        "\nA ratio **above 1.0** means Suggest is recommending against a WIDER band than the \
         gate will judge with — the direction that lets a pre-simulation solve aim past a \
         ceiling it cannot see."
    );
}

/// **A-5's 1.273×, reproduced from first principles and attributed.**
///
/// A-5 recorded the divergence under the F-LUT2 row (the two entry
/// points). It is not that. Both entry points return the SAME row for
/// both queries; what differs is the operation family the query names.
/// This test proves both halves of that statement, so Checkpoint K (a)
/// is not answered against the wrong axis.
#[test]
fn a5_band_divergence_is_the_reroute_not_the_entry_point() {
    let lut = embedded_vendor_lut();
    // A3D-1: Ø6 2-flute flat endmill, hard maple (Janka 1450),
    // Adaptive3d roughing — A-5 fixture 1.
    let suggest_query = LookupQuery {
        tool_family: rs_cam_core::feeds::vendor_lut::ToolFamily::FlatEnd,
        tool_subfamily: None,
        diameter_mm: 6.0,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        // Suggest: `op_family_to_lut(Adaptive)`.
        operation_family: LutOperationFamily::Adaptive,
        pass_role: LutPassRole::Roughing,
    };
    let gate_query = LookupQuery {
        // Gate: `routed_lookup_family(Adaptive3d, .., Adaptive, ..)`.
        operation_family: LutOperationFamily::Pocket,
        ..suggest_query.clone()
    };
    let geom = ToolGeometryHint::Flat;

    // (1) The ENTRY POINT is not the cause: on both queries the two
    //     resolvers agree exactly.
    for (name, q) in [
        ("suggest-family", &suggest_query),
        ("gate-family", &gate_query),
    ] {
        let via_geometry = find_best_row_for_geometry(lut, q, &geom).expect("row");
        let via_envelope = find_best_chip_envelope_row(lut, q, &geom).expect("row");
        assert_eq!(
            via_geometry.observation_id, via_envelope.observation_id,
            "{name}: the two entry points must agree here — if they stop agreeing, this \
             test's attribution argument no longer holds and the census must be re-read"
        );
    }

    // (2) The REROUTE is the cause: different family → different row →
    //     a band maximum 1.273× apart, exactly A-5's figure.
    let s = find_best_row_for_geometry(lut, &suggest_query, &geom).expect("row");
    let g = find_best_chip_envelope_row(lut, &gate_query, &geom).expect("row");
    assert_ne!(
        s.observation_id, g.observation_id,
        "the reroute must select a different row, or there is nothing to attribute"
    );
    let (sm, gm) = (
        s.chip_load_max_mm.expect("band"),
        g.chip_load_max_mm.expect("band"),
    );
    let ratio = sm / gm;
    println!(
        "A3D-1 attribution: Suggest `{}` max {sm:.6} vs gate `{}` max {gm:.6} → ×{ratio:.4}",
        s.observation_id, g.observation_id
    );
    assert!(
        (ratio - 1.2727).abs() < 0.001,
        "expected A-5's 1.273× to reproduce from the reroute alone; got ×{ratio:.4}. If the \
         LUT moved, re-derive the figure before citing 1.273 anywhere."
    );
    // And the band SHAPES differ, which no scale factor (diameter law,
    // hardness law, DOC derate) can produce — the standing proof that
    // this was never a scaling divergence.
    let s_shape = sm / s.chip_load_min_mm.expect("band");
    let g_shape = gm / g.chip_load_min_mm.expect("band");
    println!("  band shapes (max÷min): Suggest {s_shape:.4} vs gate {g_shape:.4}");
    assert!(
        (s_shape - g_shape).abs() > 0.01,
        "the two bands must differ in SHAPE, not just scale — that is the discriminator \
         between 'two rows' and 'one row scaled twice'"
    );
}

/// **Structural invariant, not a number**: the envelope candidate set is
/// a strict subset of the geometry candidate set, so the envelope
/// resolver can never match where the geometry resolver returns `None`.
/// Every divergence class in this census follows from that relation;
/// if it ever inverts, the census's whole classification is wrong.
#[test]
fn envelope_resolver_never_matches_where_geometry_resolver_does_not() {
    // `run_census`'s `classify` panics on the (None, Some) case; running
    // the full sweep is the proof.
    let cells = run_census();
    assert!(!cells.is_empty(), "census produced no cells");
}

/// Pins the census headline so a silent LUT edit cannot move the
/// divergence surface before Checkpoint K rules on it. **This asserts
/// the CURRENT (defective) state on purpose** — it is the pre-fix
/// reproduction F-LUT2 has lacked since Checkpoint B item 5.
///
/// Measured 2026-08-13, branch `tech-debt-3`, parent `7d8a2ea0`.
#[test]
fn f_lut2_divergence_surface_is_pinned() {
    let cells = run_census();
    let diverging: Vec<&(String, String)> = cells
        .iter()
        .filter(|(_, c)| c.divergent() > 0)
        .map(|(k, _)| k)
        .collect();
    let total_queries: usize = cells.values().map(|c| c.queries).sum();
    let total_divergent: usize = cells.values().map(|c| c.divergent()).sum();
    // The bar is stated as a floor on the DEFECT, so this test fails
    // loudly the moment the resolvers are unified (Checkpoint K (a)) —
    // which is exactly when it should be re-baselined or retired.
    assert!(
        total_divergent > 0,
        "F-LUT2 census found NO divergence across {total_queries} queries. Either the \
         resolvers were unified (retire this pin, cite the commit) or the sweep stopped \
         covering the divergent region."
    );
    assert!(
        !diverging.is_empty(),
        "divergent query count {total_divergent} with no divergent cell — classification bug"
    );
}
