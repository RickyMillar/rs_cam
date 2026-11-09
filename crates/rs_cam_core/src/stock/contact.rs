//! P22-0c-3 (`planning/feeds_contract_2026-11-03/phases/P22-0c_DESIGN.md`
//! section 5): the ONE contact kernel.
//!
//! A tool owns its zones (`tool::body::ToolBody::zones`). A contact zone
//! cuts nothing; it only touches. [`zone_contact`] tests one contact zone
//! against a [`ContactSolid`] along one move: the zone meets the solid at
//! a sample when a top inside its ring stands above the zone bottom by more
//! than the float slack. In this phase the solid is the live dexel stock of
//! the simulation walk, and the zones are `Core` and `FluteTop`
//! ([`StockContactCheck`]). The rapid arm stays in
//! [`super::collision::RapidClearanceCheck`]; [`WalkChecks`] holds both
//! for the walk.
//!
//! The result types stay in [`super::collision`]: one
//! [`NonCentreCuttingDescent`] per core contact
//! (`safety.non_centre_cutting_descent`) and one [`FluteOverreach`] per
//! flute-top contact (`tool.flute_overreach`). Their ids and texts do not
//! change (design section 5.2).

use crate::geo::P3;
use crate::tool::MillingCutter;
use crate::tool::body::{ContactKind, ContactRadius, ContactZone, CoreZone, Zone};

use super::collision::{
    FluteOverreach, NonCentreCuttingDescent, RapidClearanceCheck, RapidCollision,
    low_channel_epsilon_mm,
};

/// A solid that a contact zone can meet: the material tops on a grid of
/// columns. The live dexel stock is the one solid of this phase; the model
/// mesh joins in P22-0c-5.
pub trait ContactSolid {
    /// The highest top the solid can hold (mm). A sample whose zone bottom
    /// is at or above it meets nothing; the kernel skips it.
    fn ceiling_z(&self) -> f64;
    /// The edge of one column (mm). The kernel never samples a move closer
    /// than one column.
    fn column_mm(&self) -> f64;
    /// Visit each column whose centre is inside the disc of `radius`
    /// around `(x, y)` and that holds material: `visit` gets the column
    /// centre and the material top.
    fn for_each_top_in_disc(&self, x: f64, y: f64, radius: f64, visit: impl FnMut(f64, f64, f64));
}

impl ContactSolid for crate::dexel_stock::TriDexelStock {
    fn ceiling_z(&self) -> f64 {
        self.stock_bbox.max.z
    }

    fn column_mm(&self) -> f64 {
        self.z_grid.cell_size
    }

    fn for_each_top_in_disc(&self, x: f64, y: f64, radius: f64, visit: impl FnMut(f64, f64, f64)) {
        crate::dexel_stock::TriDexelStock::for_each_top_in_disc(self, x, y, radius, visit);
    }
}

/// P22-0b's exact in-move rule (G-SIMCORE): what the cutting ring of this
/// move removes before a sample. The ring is the band from `inner_mm` (the
/// outer radius of the zone) to the cutter radius. A column under the zone
/// takes the lower of its top before the move and the lowest cut of the
/// ring over it before the sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveSweep {
    /// The cutter radius `R` (mm).
    radius_mm: f64,
    /// The removal surface over the tip at the ring inner edge (mm).
    h_inner_mm: f64,
    /// The removal surface over the tip at the cutter edge (mm).
    h_edge_mm: f64,
}

impl MoveSweep {
    /// The sweep of the cutting ring `inner_mm ..= R` of `cutter`.
    #[must_use]
    pub fn of(cutter: &dyn MillingCutter, inner_mm: f64) -> Self {
        let h_inner_mm = cutter.height_at_radius(inner_mm).unwrap_or(0.0);
        Self {
            radius_mm: cutter.radius(),
            h_inner_mm,
            h_edge_mm: cutter
                .height_at_radius(cutter.radius())
                .unwrap_or(h_inner_mm),
        }
    }
}

/// One contact zone, ready for the kernel: the ring, the bottom over the
/// tip, the slack, and the in-move rule (`None`: the solid before the move).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneProbe {
    /// The part the zone is.
    pub kind: ContactKind,
    /// The inner radius of the ring (mm).
    pub inner_mm: f64,
    /// The outer radius of the ring (mm).
    pub outer_mm: f64,
    /// The zone bottom over the tool tip (mm).
    pub bottom_mm: f64,
    /// The stated slack of the zone (mm) on top of the float slack:
    /// `NON_CENTRE_CUTTING_TOL_MM` for the core, 0 for the flute top.
    pub tol_mm: f64,
    /// The in-move rule, or `None` for the solid as it stands before the
    /// move.
    pub sweep: Option<MoveSweep>,
}

impl ZoneProbe {
    /// The probe of `zone` on `cutter` (design section 5.2): the core
    /// reads the in-move rule (`MoveSweep`) with its slack; the flute top
    /// reads the solid before the move (design 5.3: "the FluteTop arm keeps
    /// the pre-move stock"). `None` for a zone this phase does not test on
    /// the live stock (shank, holder, shoe: P22-0c-4) and for a shelf.
    #[must_use]
    pub fn of(zone: &ContactZone, cutter: &dyn MillingCutter) -> Option<Self> {
        let outer_mm = match zone.outer {
            ContactRadius::Mm(r) => r,
            ContactRadius::Shelf => return None,
        };
        let (tol_mm, sweep) = match zone.kind {
            ContactKind::Core => (
                crate::compute::tool_config::NON_CENTRE_CUTTING_TOL_MM,
                Some(MoveSweep::of(cutter, outer_mm)),
            ),
            ContactKind::FluteTop => (0.0, None),
            // P22-0c-4 (needs Q1, Q2): fed moves against the live stock.
            ContactKind::Shank | ContactKind::Holder | ContactKind::Shoe => return None,
        };
        Some(Self {
            kind: zone.kind,
            inner_mm: zone.inner_mm,
            outer_mm,
            bottom_mm: zone.span.bottom_mm,
            tol_mm,
            sweep,
        })
    }

    /// Can this move meet the solid at all? Only a move whose lowest tip
    /// goes more than the zone bottom below the solid ceiling. The test
    /// reads no column, so a move that is not a candidate pays no flush.
    #[must_use]
    pub fn candidate(&self, solid: &impl ContactSolid, start: P3, end: P3) -> bool {
        start.z.min(end.z) < solid.ceiling_z() - self.bottom_mm
    }
}

/// THE kernel: the first sample of the move `start -> end` where `probe`
/// meets `solid`, as `(height of the material over the tip, tip)`; `None`
/// when the zone meets nothing.
///
/// The samples are at most 1 mm apart, no closer than one column (and no
/// farther than half the zone radius), the end included. At a sample a
/// column under the ring (`inner <= |q - tip| <= outer`, by column centre)
/// has the top the solid holds, or with a sweep the lower of that and the
/// lowest cut of this move's ring before the sample:
///
/// - `|q - p(0)| < outer`: no blade passed over `q` in this move; the top
///   stands.
/// - else: the distance `|q - p(s)|` is convex in `s`, so it falls through
///   `outer` once in `[0, t]`, at `s1`. The ring cut `q` from `s_in` (where
///   `q` came inside the cutter radius, or the move start) to `s1`. The tip
///   Z is linear in `s`, so the lowest cut is at one end.
///
/// The zone meets the solid where such a top stands over the tip by more
/// than the zone bottom, the stated slack and the float slack of the
/// stored tops. The answer is the highest such top at the first sample.
#[must_use]
pub fn zone_contact(
    probe: &ZoneProbe,
    solid: &impl ContactSolid,
    start: P3,
    end: P3,
) -> Option<(f64, P3)> {
    let outer = probe.outer_mm;
    let outer_sq = outer * outer;
    let inner_sq = probe.inner_mm * probe.inner_mm;
    let bottom = probe.bottom_mm;
    let tol = probe.tol_mm;
    let ceiling = solid.ceiling_z();
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let dz = end.z - start.z;
    let len_sq = dx * dx + dy * dy;
    let dxy = len_sq.sqrt();
    let step = (outer / 2.0).min(1.0).max(solid.column_mm());
    let n = crate::util::steps::steps(dxy, step).unwrap_or(0).max(1);
    let z_at = |s: f64| start.z + dz * s;
    for k in 1..=n {
        let t = k as f64 / n as f64;
        let tip = P3::new(start.x + dx * t, start.y + dy * t, z_at(t));
        if tip.z + bottom >= ceiling {
            continue;
        }
        let mut over_max = f64::NEG_INFINITY;
        solid.for_each_top_in_disc(tip.x, tip.y, outer, |x, y, top| {
            // Cheap reject: this column cannot meet the zone at its top.
            if top - tip.z <= bottom + tol {
                return;
            }
            let (tu, tv) = (x - tip.x, y - tip.y);
            if tu * tu + tv * tv < inner_sq {
                return;
            }
            let cut_top = match probe.sweep {
                None => top,
                Some(sweep) => {
                    let (qu, qv) = (x - start.x, y - start.y);
                    let d0_sq = qu * qu + qv * qv;
                    if d0_sq < outer_sq || len_sq < 1e-18 {
                        top
                    } else {
                        let radius_sq = sweep.radius_mm * sweep.radius_mm;
                        let t0 = (qu * dx + qv * dy) / len_sq;
                        let perp_sq = (d0_sq - t0 * t0 * len_sq).max(0.0);
                        let s1 =
                            (t0 - ((outer_sq - perp_sq).max(0.0) / len_sq).sqrt()).clamp(0.0, t);
                        let s_in =
                            (t0 - ((radius_sq - perp_sq).max(0.0) / len_sq).sqrt()).clamp(0.0, s1);
                        let ring = (z_at(s1) + sweep.h_inner_mm).min(z_at(s_in) + sweep.h_edge_mm);
                        top.min(ring)
                    }
                }
            };
            let over = cut_top - tip.z;
            if over > bottom + tol + low_channel_epsilon_mm(cut_top) {
                over_max = over_max.max(over);
            }
        });
        if over_max > f64::NEG_INFINITY {
            return Some((over_max, tip));
        }
    }
    None
}

/// The material top under the core disc of `core` at `(x, y)` in `stock`
/// (P22-0c-3, lead ruling on item 2): the kernel's own column read (every
/// column whose centre is inside the core radius), so the static plunge
/// check and the kernel read the same wood. Only the wood under the core
/// meets the core; the blade ring cuts the rest. The stock bottom when the
/// point is on the stock and the disc holds no material; `None` off the
/// stock (the caller then keeps its own top).
#[must_use]
pub fn core_material_top(
    stock: &crate::dexel_stock::TriDexelStock,
    core: &CoreZone,
    x: f64,
    y: f64,
) -> Option<f64> {
    let b = &stock.stock_bbox;
    if x < b.min.x || x > b.max.x || y < b.min.y || y > b.max.y {
        return None;
    }
    let mut top = b.min.z;
    ContactSolid::for_each_top_in_disc(stock, x, y, core.radius_mm, |_, _, t| {
        top = top.max(t);
    });
    Some(top)
}

/// The probes of the contact zones of `cutter` that this phase tests on
/// the live stock, from the tool body (`tool::body::body_of`): the core
/// (a tool that does not cut at its centre) and the flute top.
fn stock_probes(cutter: &dyn MillingCutter) -> Vec<ZoneProbe> {
    crate::tool::body::body_of(cutter)
        .zones()
        .iter()
        .filter_map(|zone| match zone {
            Zone::Contact(contact) => ZoneProbe::of(contact, cutter),
            Zone::Cutting(_) => None,
        })
        .collect()
}

/// The flute-top probe of `cutter` (K5 ReachFlute).
fn flute_top_probe(cutter: &dyn MillingCutter) -> Option<ZoneProbe> {
    stock_probes(cutter)
        .into_iter()
        .find(|probe| probe.kind == ContactKind::FluteTop)
}

/// The contact zones of one move that need the flushed stock: the ones
/// that meet the stock as it stands with the stamps still queued. Queued
/// stamps only remove wood, so a zone that meets nothing here meets
/// nothing on the flushed stock.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MoveContacts {
    /// One flag per probe of the check, in its order.
    flagged: Vec<bool>,
}

impl MoveContacts {
    /// Does any zone need the flushed stock?
    #[must_use]
    pub fn any(&self) -> bool {
        self.flagged.iter().any(|f| *f)
    }
}

/// The contact zones of one tool on the live stock of the simulation walk
/// (P22-0c-3): the core and the flute top, from the tool body. The walk
/// asks [`Self::unflushed`] on the stock with the stamps still queued, then
/// [`Self::check`] on the flushed stock for the zones that met it.
#[derive(Debug, Clone)]
pub struct StockContactCheck {
    probes: Vec<ZoneProbe>,
    /// The body core, for the core record (`None`: no core zone).
    core: Option<CoreZone>,
    core_hits: Vec<NonCentreCuttingDescent>,
    flute_hits: Vec<FluteOverreach>,
}

impl StockContactCheck {
    /// The check of `cutter`'s contact zones.
    #[must_use]
    pub fn new(cutter: &dyn MillingCutter) -> Self {
        Self {
            probes: stock_probes(cutter),
            core: crate::tool::body::core_of(cutter),
            core_hits: Vec::new(),
            flute_hits: Vec::new(),
        }
    }

    /// The zones of the fed move `start -> end` that meet `solid` as it
    /// stands now (the stamps still queued).
    #[must_use]
    pub fn unflushed(&self, solid: &impl ContactSolid, start: P3, end: P3) -> MoveContacts {
        MoveContacts {
            flagged: self
                .probes
                .iter()
                .map(|probe| {
                    probe.candidate(solid, start, end)
                        && zone_contact(probe, solid, start, end).is_some()
                })
                .collect(),
        }
    }

    /// Test the flagged zones of move `move_index` on the flushed `solid`
    /// and record each contact.
    pub fn check(
        &mut self,
        solid: &impl ContactSolid,
        move_index: usize,
        start: P3,
        end: P3,
        flagged: &MoveContacts,
    ) {
        for (probe, flag) in self.probes.iter().zip(&flagged.flagged) {
            if !*flag {
                continue;
            }
            let Some(strike) = zone_contact(probe, solid, start, end) else {
                continue;
            };
            match probe.kind {
                ContactKind::Core => {
                    record_core(
                        &mut self.core_hits,
                        self.core,
                        move_index,
                        start,
                        end,
                        strike,
                    );
                }
                ContactKind::FluteTop => {
                    record_flute(&mut self.flute_hits, probe, move_index, strike);
                }
                // `ZoneProbe::of` gives no probe for these in this phase.
                ContactKind::Shank | ContactKind::Holder | ContactKind::Shoe => {}
            }
        }
    }

    /// Take the flute-top contacts recorded so far.
    pub fn take_flute_hits(&mut self) -> Vec<FluteOverreach> {
        std::mem::take(&mut self.flute_hits)
    }

    /// Take the core contacts recorded so far.
    pub fn take_core_hits(&mut self) -> Vec<NonCentreCuttingDescent> {
        std::mem::take(&mut self.core_hits)
    }
}

/// G-CENTRECUT: record a core contact of move `move_index` (one per fed
/// segment: an arc move gives one per linearised window that strikes, as
/// before P22-0c-3).
fn record_core(
    hits: &mut Vec<NonCentreCuttingDescent>,
    core: Option<CoreZone>,
    move_index: usize,
    start: P3,
    end: P3,
    strike: (f64, P3),
) {
    let Some(core) = core else {
        return;
    };
    let (material_over_tip_mm, position) = strike;
    let dxy = (end.x - start.x).hypot(end.y - start.y);
    hits.push(NonCentreCuttingDescent {
        move_index,
        position,
        material_over_tip_mm,
        max_plunge_depth_mm: core.protrusion_mm,
        descent_angle_deg: (start.z - end.z).atan2(dxy).to_degrees(),
        dwell_window_mm: core.diameter_mm(),
    });
}

/// K5 ReachFlute: record a flute-top contact of move `move_index`. One
/// record per move: a later window of the same arc move adds none.
fn record_flute(
    hits: &mut Vec<FluteOverreach>,
    probe: &ZoneProbe,
    move_index: usize,
    strike: (f64, P3),
) {
    if hits.last().is_some_and(|hit| hit.move_index == move_index) {
        return;
    }
    let (material_over_tip_mm, position) = strike;
    hits.push(FluteOverreach {
        move_index,
        position,
        material_over_tip_mm,
        flute_length_mm: probe.bottom_mm,
    });
}

/// K5 ReachFlute for a drill entry, whose removal is analytic and never
/// enters the replay walk: per hole, the flute-top zone of `cutter` against
/// `stock` before the drill carves, at the hole bottom. One record per
/// hole where the zone meets wood; `move_index` is the lowest fed move of
/// `toolpath` at the hole, else 0.
#[must_use]
pub fn drill_flute_overreach(
    stock: &crate::dexel_stock::TriDexelStock,
    drill: &crate::ops::drill_op::DrillOp,
    cutter: &dyn MillingCutter,
    toolpath: &crate::toolpath::Toolpath,
) -> Vec<FluteOverreach> {
    let Some(probe) = flute_top_probe(cutter) else {
        return Vec::new();
    };
    let radius = probe.outer_mm;
    drill
        .holes
        .iter()
        .filter_map(|hole| {
            let tip = P3::new(hole.xy[0], hole.xy[1], hole.bottom_z);
            let (over, _) = zone_contact(&probe, stock, tip, tip)?;
            let move_index = toolpath
                .moves
                .iter()
                .enumerate()
                .filter(|(_, m)| m.move_type != crate::toolpath::MoveType::Rapid)
                .filter(|(_, m)| (m.target.x - tip.x).hypot(m.target.y - tip.y) <= radius)
                .min_by(|(_, a), (_, b)| a.target.z.total_cmp(&b.target.z))
                .map_or(0, |(i, _)| i);
            Some(FluteOverreach {
                move_index,
                position: tip,
                material_over_tip_mm: over,
                flute_length_mm: probe.bottom_mm,
            })
        })
        .collect()
}

/// The checks that ride the simulation walk (P22-0c-3): the rapid arm and
/// the contact zones of the tool on the live stock.
pub struct WalkChecks<'a> {
    /// The rapid arm (`project.rapid_collision`).
    pub rapid: RapidClearanceCheck<'a>,
    /// The contact zones on fed moves (`safety.non_centre_cutting_descent`,
    /// `tool.flute_overreach`).
    pub contact: StockContactCheck,
}

/// What the walk checks found on one toolpath.
#[derive(Debug, Clone, Default)]
pub struct WalkFindings {
    /// The struck rapids.
    pub rapids: Vec<RapidCollision>,
    /// The core contacts.
    pub core: Vec<NonCentreCuttingDescent>,
    /// The flute-top contacts.
    pub flute: Vec<FluteOverreach>,
}

impl<'a> WalkChecks<'a> {
    /// The checks of `cutter`.
    #[must_use]
    pub fn new(cutter: &'a dyn MillingCutter) -> Self {
        Self {
            rapid: RapidClearanceCheck::new(cutter),
            contact: StockContactCheck::new(cutter),
        }
    }

    /// Consume the checks and take every finding.
    #[must_use]
    pub fn into_findings(mut self) -> WalkFindings {
        WalkFindings {
            core: self.contact.take_core_hits(),
            flute: self.contact.take_flute_hits(),
            rapids: self.rapid.into_hits(),
        }
    }
}
