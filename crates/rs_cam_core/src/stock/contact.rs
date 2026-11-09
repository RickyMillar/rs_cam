//! P22-0c-3 (`planning/feeds_contract_2026-11-03/phases/P22-0c_DESIGN.md`
//! section 5): the ONE contact kernel.
//!
//! A tool owns its zones (`tool::body::ToolBody::zones`). A contact zone
//! cuts nothing; it only touches. [`zone_contact`] tests one contact zone
//! against a [`ContactSolid`] along one move: the zone meets the solid at
//! a sample when a top inside its ring stands above the zone bottom by more
//! than the float slack. The solid is the live dexel stock of the
//! simulation walk ([`StockContactCheck`]). On a fed move every contact
//! zone of the mounted tool is tested: `Core` and `FluteTop` (P22-0c-3),
//! and `Shank`, `Holder` and `Shoe` (P22-0c-4). On a rapid every
//! non-cutting zone above the flutes is tested; the envelope arm stays in
//! [`super::collision::RapidClearanceCheck`]. [`WalkChecks`] holds both
//! for the walk.
//!
//! P22-0c-4 shadow rule: a zone at or above the flute top whose radius is
//! no more than the cutter envelope radius meets wood only where the flute
//! top (fed) or the envelope (rapid) meets it first, at the same sample.
//! It gets no probe, so a plain end mill (shank no wider than the cutter)
//! adds no finding. A wider zone at or above the flute top reads only its
//! ring outside the envelope radius, for the same reason.
//!
//! The result types stay in [`super::collision`]: one
//! [`NonCentreCuttingDescent`] per core contact
//! (`safety.non_centre_cutting_descent`) and one [`FluteOverreach`] per
//! flute-top contact (`tool.flute_overreach`). Their ids and texts do not
//! change (design section 5.2).

use crate::geo::P3;
use crate::tool::MillingCutter;
use crate::tool::body::{ContactKind, ContactRadius, ContactZone, CoreZone, MountedBody, Zone};

use super::collision::{
    BodyContact, ContactMotion, FluteOverreach, NonCentreCuttingDescent, RapidClearanceCheck,
    RapidCollision, low_channel_epsilon_mm,
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

/// Which stock a contact zone at or above the flute top reads on a fed
/// move (P22-0c-4, operator ruling Q2). The core always reads P22-0b's
/// ring rule ([`MoveSweep`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactRule {
    /// The stock as it stands before the move (P22-0c-3).
    PreMove,
    /// The stock less what the flutes of this move cut before the sample
    /// ([`DiscSweep`]).
    InMove,
}

/// THE rule of the zones at or above the flute top on a fed move
/// (P22-0c-4, ruling Q2). The count on the `planning/fixtures/` projects
/// (`planning/feeds_contract_2026-11-03/phases/P22-0c-4.md`) found no
/// report under either rule, so no removed report was shown to be false.
/// The conservative rule stays: the stock before the move (every report
/// of P22-0c-3 is kept). [`ContactRule::InMove`] is built and tested.
pub const SHIPPED_RULE: ContactRule = ContactRule::PreMove;

/// The test-support override of [`SHIPPED_RULE`] (0: none, 1: pre-move,
/// 2: in-move), for the Q2 count only.
#[cfg(feature = "test-support")]
static RULE_OVERRIDE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Test support (the Q2 count, P22-0c-4): every [`StockContactCheck`]
/// built after this call reads `rule`; `None` gives [`SHIPPED_RULE`] back.
/// Process-wide: a caller runs one rule at a time.
#[cfg(feature = "test-support")]
pub fn override_rule_for_count(rule: Option<ContactRule>) {
    let value = match rule {
        None => 0,
        Some(ContactRule::PreMove) => 1,
        Some(ContactRule::InMove) => 2,
    };
    RULE_OVERRIDE.store(value, std::sync::atomic::Ordering::SeqCst);
}

/// The rule a new check reads: [`SHIPPED_RULE`], or the test-support
/// override.
fn current_rule() -> ContactRule {
    #[cfg(feature = "test-support")]
    {
        match RULE_OVERRIDE.load(std::sync::atomic::Ordering::SeqCst) {
            1 => return ContactRule::PreMove,
            2 => return ContactRule::InMove,
            _ => {}
        }
    }
    SHIPPED_RULE
}

/// P22-0c-4 (ruling Q2): the in-move rule of a zone at or above the flute
/// top. The flutes cut every column that comes within the cutter radius
/// `R` (outside the core radius `c` of a core tool) up to the flute top.
/// A column can be under the zone at a sample after the flutes of this
/// move cut it; it then takes the lower of its top before the move and an
/// upper bound of that cut.
///
/// For a column `q`, `[e1, e2]` is the part of `[0, t]` in which `q` is
/// within `R` (`|q - p(s)|` is convex, so it is one interval). The cut at
/// a time `e` is at most `z(e) + h(R)`: the profile does not fall with the
/// radius. The flutes reach `q` only when its top is at most the flute
/// length over the tip; else the flute top meets it and the top stands.
///
/// - `c = 0`: the flutes cut `q` all through `[e1, e2]`, so once they
///   reach it (top at most `max(z(e1), z(e2)) + L`) its top stays at most
///   `z(s) + h(R)` to `e2`. The cut is `z(e2) + h(R)`.
/// - `c > 0`: `q` can pass under the core, which cuts nothing, so only an
///   end of `[e1, e2]` where `q` is in the ring counts, each with its own
///   reach test. Conservative: the cut is never lower than the true one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscSweep {
    /// The cutter radius `R` (mm).
    radius_mm: f64,
    /// The core radius `c` (mm); 0 for a tool that cuts at its centre.
    core_mm: f64,
    /// The removal surface over the tip at the cutter edge, `h(R)` (mm).
    h_edge_mm: f64,
    /// The flute length `L` (mm).
    flute_mm: f64,
}

impl DiscSweep {
    /// The in-move rule of `cutter` (its radius, its flute length and the
    /// core radius of its body).
    #[must_use]
    pub fn of(cutter: &dyn MillingCutter) -> Self {
        let radius_mm = cutter.radius();
        Self {
            radius_mm,
            core_mm: crate::tool::body::core_of(cutter).map_or(0.0, |core| core.radius_mm),
            h_edge_mm: cutter.height_at_radius(radius_mm).unwrap_or(0.0),
            flute_mm: cutter.length(),
        }
    }

    /// The top of the column at `(x, y)` (top before the move `top`) at
    /// the sample `t` of the move `start -> start + (dx, dy, dz)`.
    #[must_use]
    fn cut_top(&self, top: f64, x: f64, y: f64, start: P3, d: (f64, f64, f64), t: f64) -> f64 {
        let (dx, dy, dz) = d;
        let z_at = |s: f64| start.z + dz * s;
        let (qu, qv) = (x - start.x, y - start.y);
        let d0_sq = qu * qu + qv * qv;
        let len_sq = dx * dx + dy * dy;
        let r_sq = self.radius_mm * self.radius_mm;
        let c_sq = self.core_mm * self.core_mm;
        // The interval of [0, t] in which the column is within R. A move
        // with no XY keeps the column at its start distance.
        let (e1, e2, t0, perp_sq) = if len_sq < 1e-18 {
            if d0_sq > r_sq {
                return top;
            }
            (0.0, t, 0.0, d0_sq)
        } else {
            let t0 = (qu * dx + qv * dy) / len_sq;
            let perp_sq = (d0_sq - t0 * t0 * len_sq).max(0.0);
            if perp_sq > r_sq {
                return top;
            }
            let half = ((r_sq - perp_sq) / len_sq).sqrt();
            ((t0 - half).max(0.0), (t0 + half).min(t), t0, perp_sq)
        };
        if e1 > e2 {
            return top;
        }
        let dist_sq = |s: f64| perp_sq + (s - t0) * (s - t0) * len_sq;
        let reaches = |e: f64| top <= z_at(e) + self.flute_mm;
        let cut = if self.core_mm <= 0.0 {
            if top <= z_at(e1).max(z_at(e2)) + self.flute_mm {
                z_at(e2) + self.h_edge_mm
            } else {
                f64::INFINITY
            }
        } else {
            [e1, e2]
                .into_iter()
                .filter(|&e| dist_sq(e) >= c_sq && reaches(e))
                .map(|e| z_at(e) + self.h_edge_mm)
                .fold(f64::INFINITY, f64::min)
        };
        top.min(cut)
    }
}

/// The in-move rule a probe reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoneSweep {
    /// The core: P22-0b's ring rule.
    Core(MoveSweep),
    /// A zone at or above the flute top (P22-0c-4, ruling Q2).
    Disc(DiscSweep),
}

/// One contact zone, ready for the kernel: the ring, the bottom over the
/// tip, the slack, and the in-move rule (`None`: the solid before the move).
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneProbe {
    /// The part the zone is.
    pub kind: ContactKind,
    /// The head part name (`None` for a tool zone).
    pub name: Option<String>,
    /// The inner radius of the ring (mm).
    pub inner_mm: f64,
    /// The outer radius of the ring (mm).
    pub outer_mm: f64,
    /// The zone bottom over the tool tip (mm).
    pub bottom_mm: f64,
    /// The stated slack of the zone (mm) on top of the float slack:
    /// `NON_CENTRE_CUTTING_TOL_MM` for the core, 0 for every other zone.
    pub tol_mm: f64,
    /// The in-move rule, or `None` for the solid as it stands before the
    /// move.
    pub sweep: Option<ZoneSweep>,
}

impl ZoneProbe {
    /// The probe of `zone` on `cutter` for a move of `motion` under
    /// `rule` (design section 5.2), or `None` when the zone gives no
    /// probe:
    ///
    /// - The core (fed only) reads the ring rule with its slack. On a
    ///   rapid the envelope arm covers it.
    /// - The flute top (fed only) reads `rule`. On a rapid the envelope
    ///   arm covers it.
    /// - The shank, a holder band and a shoe part read `rule` on a fed
    ///   move and the stock before the move on a rapid (a rapid cuts
    ///   nothing). The shadow rule (module text) skips a zone at or above
    ///   the flute top no wider than the envelope, and gives a wider one
    ///   the ring outside the envelope. A shelf (no width limit) gives no
    ///   probe: the H1 head check reads it against the stock box.
    #[must_use]
    pub fn of(
        zone: &ContactZone,
        cutter: &dyn MillingCutter,
        rule: ContactRule,
        motion: ContactMotion,
    ) -> Option<Self> {
        let outer_mm = match zone.outer {
            ContactRadius::Mm(r) => r,
            ContactRadius::Shelf => return None,
        };
        let disc = || match rule {
            ContactRule::PreMove => None,
            ContactRule::InMove => Some(ZoneSweep::Disc(DiscSweep::of(cutter))),
        };
        let probe = |inner_mm: f64, tol_mm: f64, sweep: Option<ZoneSweep>| {
            Some(Self {
                kind: zone.kind,
                name: zone.name.clone(),
                inner_mm,
                outer_mm,
                bottom_mm: zone.span.bottom_mm,
                tol_mm,
                sweep,
            })
        };
        match (zone.kind, motion) {
            (ContactKind::Core, ContactMotion::Fed) => probe(
                zone.inner_mm,
                crate::compute::tool_config::NON_CENTRE_CUTTING_TOL_MM,
                Some(ZoneSweep::Core(MoveSweep::of(cutter, outer_mm))),
            ),
            (ContactKind::FluteTop, ContactMotion::Fed) => probe(zone.inner_mm, 0.0, disc()),
            (ContactKind::Core | ContactKind::FluteTop, ContactMotion::Rapid) => None,
            (ContactKind::Shank | ContactKind::Holder | ContactKind::Shoe, _) => {
                let envelope = cutter.envelope_radius_mm();
                let above_flutes = zone.span.bottom_mm >= cutter.length();
                if above_flutes && outer_mm <= envelope {
                    return None;
                }
                let inner_mm = if above_flutes {
                    zone.inner_mm.max(envelope)
                } else {
                    zone.inner_mm
                };
                let sweep = match motion {
                    ContactMotion::Fed => disc(),
                    ContactMotion::Rapid => None,
                };
                probe(inner_mm, 0.0, sweep)
            }
        }
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
                Some(ZoneSweep::Disc(disc)) => disc.cut_top(top, x, y, start, (dx, dy, dz), t),
                Some(ZoneSweep::Core(sweep)) => {
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

/// The probes of the contact `zones` of `cutter` for a move of `motion`
/// under `rule` ([`ZoneProbe::of`]).
fn probes_of(
    zones: &[Zone],
    cutter: &dyn MillingCutter,
    rule: ContactRule,
    motion: ContactMotion,
) -> Vec<ZoneProbe> {
    zones
        .iter()
        .filter_map(|zone| match zone {
            Zone::Contact(contact) => ZoneProbe::of(contact, cutter, rule, motion),
            Zone::Cutting(_) => None,
        })
        .collect()
}

/// The flute-top probe of `cutter` (K5 ReachFlute), on the stock before
/// the move (a drill hole has no move to sweep).
fn flute_top_probe(cutter: &dyn MillingCutter) -> Option<ZoneProbe> {
    probes_of(
        &crate::tool::body::body_of(cutter).zones(),
        cutter,
        ContactRule::PreMove,
        ContactMotion::Fed,
    )
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

/// The contact zones of one mounted tool on the live stock of the
/// simulation walk: on a fed move the core, the flute top (P22-0c-3), the
/// shank, the holder and the shoe (P22-0c-4); on a rapid the non-cutting
/// zones above the flutes. The walk asks [`Self::unflushed`] (or
/// [`Self::rapid_unflushed`]) on the stock with the stamps still queued,
/// then [`Self::check`] (or [`Self::check_rapid`]) on the flushed stock for
/// the zones that met it.
#[derive(Debug, Clone)]
pub struct StockContactCheck {
    probes: Vec<ZoneProbe>,
    rapid_probes: Vec<ZoneProbe>,
    /// The body core, for the core record (`None`: no core zone).
    core: Option<CoreZone>,
    core_hits: Vec<NonCentreCuttingDescent>,
    flute_hits: Vec<FluteOverreach>,
    body_hits: Vec<BodyContact>,
}

impl StockContactCheck {
    /// The check of `cutter`'s own contact zones (the tool body; no head
    /// bands).
    #[must_use]
    pub fn new(cutter: &dyn MillingCutter) -> Self {
        Self::of_zones(&crate::tool::body::body_of(cutter).zones(), cutter)
    }

    /// The check of the contact zones of `mounted` (the tool body plus the
    /// head bands at its holder face), cut by `cutter`.
    #[must_use]
    pub fn mounted(cutter: &dyn MillingCutter, mounted: &MountedBody) -> Self {
        Self::of_zones(&mounted.zones(), cutter)
    }

    /// [`Self::mounted`] under `rule`, not [`SHIPPED_RULE`] (the Q2
    /// sentries compare the two rules on one toolpath).
    #[must_use]
    pub fn mounted_with_rule(
        cutter: &dyn MillingCutter,
        mounted: &MountedBody,
        rule: ContactRule,
    ) -> Self {
        Self::of_zones_with_rule(&mounted.zones(), cutter, rule)
    }

    fn of_zones(zones: &[Zone], cutter: &dyn MillingCutter) -> Self {
        Self::of_zones_with_rule(zones, cutter, current_rule())
    }

    fn of_zones_with_rule(zones: &[Zone], cutter: &dyn MillingCutter, rule: ContactRule) -> Self {
        Self {
            probes: probes_of(zones, cutter, rule, ContactMotion::Fed),
            rapid_probes: probes_of(zones, cutter, rule, ContactMotion::Rapid),
            core: crate::tool::body::core_of(cutter),
            core_hits: Vec::new(),
            flute_hits: Vec::new(),
            body_hits: Vec::new(),
        }
    }

    /// The zones of the fed move `start -> end` that meet `solid` as it
    /// stands now (the stamps still queued).
    #[must_use]
    pub fn unflushed(&self, solid: &impl ContactSolid, start: P3, end: P3) -> MoveContacts {
        flag(&self.probes, solid, start, end)
    }

    /// The zones of the rapid `start -> end` that meet `solid` as it
    /// stands now. A rapid straight up meets nothing: the body at a higher
    /// tip meets a subset of the columns it met below (the reason the
    /// envelope arm exempts it).
    #[must_use]
    pub fn rapid_unflushed(&self, solid: &impl ContactSolid, start: P3, end: P3) -> MoveContacts {
        if super::collision::rapid_is_straight_up(start, end) {
            return MoveContacts {
                flagged: vec![false; self.rapid_probes.len()],
            };
        }
        flag(&self.rapid_probes, solid, start, end)
    }

    /// Test the flagged zones of the fed move `move_index` on the flushed
    /// `solid` and record each contact.
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
                ContactKind::Shank | ContactKind::Holder | ContactKind::Shoe => {
                    record_body(
                        &mut self.body_hits,
                        probe,
                        move_index,
                        strike,
                        ContactMotion::Fed,
                    );
                }
            }
        }
    }

    /// Test the flagged zones of the rapid `move_index` on the flushed
    /// `solid` and record each contact. True when a zone met the stock.
    pub fn check_rapid(
        &mut self,
        solid: &impl ContactSolid,
        move_index: usize,
        start: P3,
        end: P3,
        flagged: &MoveContacts,
    ) -> bool {
        let mut met = false;
        for (probe, flag) in self.rapid_probes.iter().zip(&flagged.flagged) {
            if !*flag {
                continue;
            }
            if let Some(strike) = zone_contact(probe, solid, start, end) {
                record_body(
                    &mut self.body_hits,
                    probe,
                    move_index,
                    strike,
                    ContactMotion::Rapid,
                );
                met = true;
            }
        }
        met
    }

    /// Take the flute-top contacts recorded so far.
    pub fn take_flute_hits(&mut self) -> Vec<FluteOverreach> {
        std::mem::take(&mut self.flute_hits)
    }

    /// Take the core contacts recorded so far.
    pub fn take_core_hits(&mut self) -> Vec<NonCentreCuttingDescent> {
        std::mem::take(&mut self.core_hits)
    }

    /// Take the shank, holder and shoe contacts recorded so far.
    pub fn take_body_hits(&mut self) -> Vec<BodyContact> {
        std::mem::take(&mut self.body_hits)
    }
}

/// The flags of `probes` on `solid` for the move `start -> end`.
fn flag(probes: &[ZoneProbe], solid: &impl ContactSolid, start: P3, end: P3) -> MoveContacts {
    MoveContacts {
        flagged: probes
            .iter()
            .map(|probe| {
                probe.candidate(solid, start, end)
                    && zone_contact(probe, solid, start, end).is_some()
            })
            .collect(),
    }
}

/// P22-0c-4: record a shank, holder or shoe contact of move `move_index`.
/// One record per move per zone: a later window of the same arc move adds
/// none.
fn record_body(
    hits: &mut Vec<BodyContact>,
    probe: &ZoneProbe,
    move_index: usize,
    strike: (f64, P3),
    motion: ContactMotion,
) {
    if hits
        .iter()
        .rev()
        .take_while(|hit| hit.move_index == move_index)
        .any(|hit| hit.zone == probe.kind && hit.name == probe.name)
    {
        return;
    }
    let (material_over_tip_mm, position) = strike;
    hits.push(BodyContact {
        move_index,
        position,
        zone: probe.kind,
        name: probe.name.clone(),
        zone_bottom_mm: probe.bottom_mm,
        material_over_tip_mm,
        motion,
    });
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
    /// The contact zones (`safety.non_centre_cutting_descent`,
    /// `tool.flute_overreach`, `project.holder_collision`; on a rapid
    /// `project.rapid_collision`).
    pub contact: StockContactCheck,
}

/// What the walk checks found on one toolpath.
#[derive(Debug, Clone, Default)]
pub struct WalkFindings {
    /// The struck rapids (the envelope, or a zone above the flutes).
    pub rapids: Vec<RapidCollision>,
    /// The core contacts.
    pub core: Vec<NonCentreCuttingDescent>,
    /// The flute-top contacts.
    pub flute: Vec<FluteOverreach>,
    /// The shank, holder and shoe contacts (fed and rapid).
    pub body: Vec<BodyContact>,
}

impl<'a> WalkChecks<'a> {
    /// The checks of `cutter` (its body zones; no head bands).
    #[must_use]
    pub fn new(cutter: &'a dyn MillingCutter) -> Self {
        Self {
            rapid: RapidClearanceCheck::new(cutter),
            contact: StockContactCheck::new(cutter),
        }
    }

    /// The checks of `cutter` mounted as `mounted` (the body plus the head
    /// bands at its holder face): the simulation run of a toolpath.
    #[must_use]
    pub fn mounted(cutter: &'a dyn MillingCutter, mounted: &MountedBody) -> Self {
        Self {
            rapid: RapidClearanceCheck::new(cutter),
            contact: StockContactCheck::mounted(cutter, mounted),
        }
    }

    /// [`Self::mounted`] under `rule` (the Q2 sentries).
    #[must_use]
    pub fn mounted_with_rule(
        cutter: &'a dyn MillingCutter,
        mounted: &MountedBody,
        rule: ContactRule,
    ) -> Self {
        Self {
            rapid: RapidClearanceCheck::new(cutter),
            contact: StockContactCheck::mounted_with_rule(cutter, mounted, rule),
        }
    }

    /// Consume the checks and take every finding.
    #[must_use]
    pub fn into_findings(mut self) -> WalkFindings {
        WalkFindings {
            core: self.contact.take_core_hits(),
            flute: self.contact.take_flute_hits(),
            body: self.contact.take_body_hits(),
            rapids: self.rapid.into_hits(),
        }
    }
}
