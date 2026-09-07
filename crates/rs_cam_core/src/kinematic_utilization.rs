//! Kinematic utilisation — the machine-confidence instrument (Phase 2,
//! 2026-09-07, `planning/machine_kinematics_confidence_2026-09-07.md`).
//!
//! The operator wants one reading before pressing go: how hard is the
//! machine working on this toolpath, where is the headroom, and which
//! commands can the controller not realise? This module answers that
//! from the emitted [`Toolpath`] alone. It needs no simulation.
//!
//! # What it measures
//!
//! For every move the instrument reports:
//!
//! * a motion class taken from the move's GEOMETRY ([`classify_move`]),
//! * the commanded feed and the feed the machine actually reaches,
//! * which limit bound that feed ([`KinematicBinding`]),
//! * the achieved Z-descent rate, and the plunge ratio against the
//!   operation's plunge rate for `Plunge`-class moves.
//!
//! For the toolpath it reports the time-weighted commanded and achieved
//! feeds, the utilisation ratio, the fraction of fed time each binding
//! holds, and the plunge and ramp populations.
//!
//! # One physics site
//!
//! Every peak velocity comes from
//! [`crate::machine_kinematics::move_kinematics`]. Every junction
//! velocity comes from
//! [`crate::machine_kinematics::junction_velocity`]. The instrument
//! reproduces the integrator's sequencing exactly: it skips the seed
//! move and every degenerate move, it takes an arc on its chord, and it
//! floors a rapid's commanded rate at the machine travel rate. It never
//! reimplements a trapezoid. A verdict from this instrument and the
//! cycle time the runtime integrator predicts therefore always agree
//! about what the machine does.
//!
//! # Time base
//!
//! `move_kinematics` publishes the peak velocity, not the integrated
//! time, so this instrument weights every statistic by
//! `length / achieved` — the time the move takes at its peak. That
//! under-states the accel and decel ramps. It is acceptable here
//! because both sides of every ratio use the same measure. Do NOT
//! compare `fed_time_s` against
//! [`crate::machine_kinematics::compute_cycle_time`]: that function
//! integrates the ramps and it includes rapids and the jerk penalty.
//!
//! # The `JunctionBound` caveat (Phase 1, binding)
//!
//! [`KinematicBinding::JunctionBound`] fires only when a move's peak is
//! at or below `max(v_in, v_out)`. That is an infeasible-junction
//! artefact of the forward-only integrator, which never lowers an entry
//! velocity to a value the previous move can reach. On real toolpaths
//! the variant is rare, and the mass that is not `FeedBound` reads
//! [`KinematicBinding::AccelBound`] instead. **A `junction_bound`
//! fraction of 0 % does NOT mean cornering is free.** A corner lowers
//! the junction velocities, and the next move then runs out of length,
//! so the cost of cornering appears as `AccelBound` time. Read
//! [`BindingFractions::machine_bound`] — the accel, rate and junction
//! shares together — for "the time the machine limited, not the
//! command".
//!
//! # Not measured is not zero
//!
//! Every aggregate that needs a population is an `Option`. A toolpath
//! with no fed move reports `None`, never `0.0`. Read
//! [`ToolpathKinematicUtilization::is_measured`] and
//! [`ToolpathKinematicUtilization::plunge_is_measured`] before you read
//! a number.

use serde::{Deserialize, Serialize};

use crate::ids::ToolpathId;
use crate::machine_kinematics::{
    KinematicBinding, MachineKinematics, junction_velocity, move_kinematics,
};
use crate::toolpath::{MoveType, Toolpath};

/// Half-angle (degrees, measured from VERTICAL) of the plunge class.
///
/// A descending fed move inside this cone is a plunge: the tool cuts on
/// its centre, chip evacuation is poor, and the operation's plunge rate
/// governs it. Outside the cone the flutes cut laterally and the
/// chipload band governs instead.
pub const PLUNGE_CLASS_HALF_ANGLE_DEG: f64 = 15.0;

/// Descent angle (degrees, measured from HORIZONTAL) below which a
/// descending fed move counts as lateral.
///
/// A finish pass that follows a shallow surface descends by a fraction
/// of a degree. A ramp population filled with such moves reports
/// nothing useful.
pub const LATERAL_MAX_DESCENT_ANGLE_DEG: f64 = 2.0;

/// Motion class of one move, decided by geometry alone.
///
/// The physics correction of 2026-09-07: a sloped fed descent is a
/// RAMP. Its flutes cut laterally and the chipload gate already governs
/// it, so its Z component above the plunge rate is not a hazard. Only a
/// vertical-dominant fed descent is a plunge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionClass {
    /// A descending fed move within [`PLUNGE_CLASS_HALF_ANGLE_DEG`] of
    /// vertical. Graded against the operation's plunge rate.
    Plunge,
    /// A descending fed move steeper than
    /// [`LATERAL_MAX_DESCENT_ANGLE_DEG`] and outside the plunge cone.
    Ramp,
    /// A fed move that is flat, or that descends by less than
    /// [`LATERAL_MAX_DESCENT_ANGLE_DEG`].
    Lateral,
    /// A rapid, an ascending move, or a zero-length move. Not graded.
    Retract,
}

/// Classify one move by the GEOMETRY of its vector, never by intent
/// tag.
///
/// `delta` is `target − previous target` in mm. An arc is classified on
/// its chord, which is what the runtime integrator measures too.
///
/// The decision order is:
///
/// * a rapid, an ascending move (`dz > 0`) or a zero-length move is
///   [`MotionClass::Retract`];
/// * a descending move within [`PLUNGE_CLASS_HALF_ANGLE_DEG`] of
///   vertical is [`MotionClass::Plunge`];
/// * a descending move whose descent angle from horizontal is below
///   [`LATERAL_MAX_DESCENT_ANGLE_DEG`] is [`MotionClass::Lateral`];
/// * every other move is [`MotionClass::Ramp`].
///
/// Phase 3's modulator guard reads this same function, so the guard and
/// the instrument can never disagree about what a plunge is.
pub fn classify_move(move_type: MoveType, delta: [f64; 3]) -> MotionClass {
    if matches!(move_type, MoveType::Rapid) {
        return MotionClass::Retract;
    }
    let [dx, dy, dz] = delta;
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    if !length.is_finite() || length <= 1e-9 {
        return MotionClass::Retract;
    }
    if dz > 0.0 {
        return MotionClass::Retract;
    }
    // Cosine of the angle between the move and the downward vertical.
    // Clamped because a rounded component can fall just outside [0, 1].
    let descent = (-dz / length).clamp(0.0, 1.0);
    if descent.acos().to_degrees() <= PLUNGE_CLASS_HALF_ANGLE_DEG {
        return MotionClass::Plunge;
    }
    if descent.asin().to_degrees() < LATERAL_MAX_DESCENT_ANGLE_DEG {
        MotionClass::Lateral
    } else {
        MotionClass::Ramp
    }
}

/// Whether the analysed move list carries PLANNED or EMITTED feeds
/// (Phase 3, 2026-09-07).
///
/// The instrument reads whatever move list its caller hands it, and the
/// two are not the same thing. The feed modulator runs AFTER a
/// simulation; before one, a generated toolpath carries the operation's
/// commanded feeds — the plan. A surface that prints a kinematic reading
/// without saying which one it read invites the operator to act on a
/// number that the post-processor will not emit
/// (`feedback_measure_emitted_motion`).
///
/// [`analyse_toolpath`] cannot know: it is a pure function over one move
/// list, with no session and no trace. It therefore reports the
/// CONSERVATIVE answer, [`Self::Planned`], and
/// [`crate::session::ProjectSession::kinematic_utilization_of`] — the
/// session's single producer — stamps the measured one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedsProvenance {
    /// The move list carries the operation's commanded feeds. Either no
    /// simulation has run, or the feeds on this list do not match the
    /// modulated feeds the run recorded.
    #[default]
    Planned,
    /// The move list carries the feeds the post-processor will emit: a
    /// simulation has run and every modulated feed it recorded for this
    /// toolpath is present on this list.
    Emitted,
}

impl FeedsProvenance {
    /// The parenthetical every operator surface appends to its kinematic
    /// reading, so the number is never quoted without its provenance.
    pub fn qualifier(self) -> &'static str {
        match self {
            Self::Emitted => "emitted",
            Self::Planned => "planned feeds — run a simulation for emitted",
        }
    }
}

/// One move's kinematic reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveUtilization {
    /// Index into [`Toolpath::moves`]. The seed move 0 and every
    /// degenerate move are absent, so this is not a position in
    /// [`ToolpathKinematicUtilization::moves`].
    pub move_index: usize,
    /// Geometric class of the move ([`classify_move`]).
    pub class: MotionClass,
    /// Chord length of the move (mm).
    pub length_mm: f64,
    /// The RAW commanded feed (mm/min) — the `F` word of a fed move, or
    /// the effective travel rate of a rapid. The machine's `max_feed`
    /// does NOT cap it, so a reader can see a command the machine
    /// cannot realise. A command above `max_feed` reads `FeedBound`
    /// with `achieved_mm_min` equal to `max_feed`.
    pub commanded_mm_min: f64,
    /// Peak velocity (mm/min) the machine reaches on this move.
    pub achieved_mm_min: f64,
    /// The limit that set `achieved_mm_min`. Read the module doc's
    /// `JunctionBound` caveat before you interpret this.
    pub binding: KinematicBinding,
    /// `|dz| / length × achieved_mm_min` (mm/min) on a descending move.
    /// `0.0` on a flat or ascending move. It is a rate, never a
    /// verdict.
    pub achieved_z_rate_mm_min: f64,
    /// `achieved_z_rate_mm_min / plunge_rate` on a
    /// [`MotionClass::Plunge`] move. `None` on every other class, and
    /// when the operation carries no positive plunge rate.
    pub plunge_ratio: Option<f64>,
    /// Unit direction of the move. Stored so
    /// [`ToolpathKinematicUtilization::headroom_estimate`] can re-solve
    /// the move without the toolpath.
    pub dir: [f64; 3],
    /// Junction velocity entering the move (mm/min).
    pub v_in_mm_min: f64,
    /// Junction velocity leaving the move (mm/min).
    pub v_out_mm_min: f64,
    /// True for `MoveType::Rapid`. The instrument classifies a rapid,
    /// but a rapid never contributes to the fed time, to the binding
    /// fractions, or to the headroom estimate.
    pub is_rapid: bool,
}

/// Fraction of FED time each binding held, on one toolpath.
///
/// The four named fractions sum to 1.0 whenever the toolpath carries
/// fed time. `machine_bound` is a derived roll-up, not a fifth share.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BindingFractions {
    /// Fed time the machine spent at the commanded feed. A
    /// constant-chipload feed rise can convert this share into saved
    /// time.
    pub feed_bound: f64,
    /// Fed time the acceleration limit held, because the move ran out
    /// of length. The cost of cornering appears here, not in
    /// `junction_bound`.
    pub accel_bound: f64,
    /// Fed time a per-axis maximum rate (`$110/$111/$112`) held.
    pub rate_bound: f64,
    /// Fed time an entry or exit junction velocity held. Rare — read
    /// the module doc's caveat. A zero here is NOT "cornering is free".
    pub junction_bound: f64,
    /// `accel_bound + rate_bound + junction_bound` — the fed time the
    /// MACHINE limited rather than the command. A feed rise cannot move
    /// this share.
    pub machine_bound: f64,
}

/// Plunge-class population and worst case on one toolpath.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlungeClassObservation {
    /// Number of [`MotionClass::Plunge`] moves. A `0` means the
    /// observation is ABSENT, not that the plunges are clean.
    pub population: usize,
    /// Largest [`MoveUtilization::plunge_ratio`] seen. `None` when the
    /// population is empty, or when no positive plunge rate was given.
    pub peak_ratio: Option<f64>,
    /// Plunges whose achieved Z rate is above the plunge rate.
    pub over_1x: usize,
    /// Plunges whose achieved Z rate is above twice the plunge rate.
    pub over_2x: usize,
    /// [`Toolpath::moves`] index of the worst plunge.
    pub worst_move_index: Option<usize>,
    /// End point `[x, y, z]` (mm) of the worst plunge, so the operator
    /// can find it in the viewport.
    pub worst_position: Option<[f64; 3]>,
    /// Achieved Z-descent rate (mm/min) of the worst plunge.
    pub worst_achieved_z_rate_mm_min: Option<f64>,
    /// The operation's plunge rate (mm/min) the ratios divide by. It
    /// stays on the observation so a reader never has to guess the
    /// denominator.
    pub plunge_rate_mm_min: f64,
}

/// Ramp-class population and steepest case on one toolpath.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RampObservation {
    /// Number of [`MotionClass::Ramp`] moves.
    pub population: usize,
    /// Steepest descent angle (degrees from HORIZONTAL) among them.
    /// `None` when the population is empty.
    pub steepest_deg_from_horizontal: Option<f64>,
    /// [`Toolpath::moves`] index of the steepest ramp.
    pub worst_move_index: Option<usize>,
}

/// One toolpath's kinematic utilisation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathKinematicUtilization {
    /// Identity of the analysed toolpath.
    pub toolpath_id: ToolpathId,
    /// `Toolpath::moves.len()`, the seed move included.
    pub moves_total: usize,
    /// Non-degenerate fed moves that resolved to a positive achieved
    /// feed. This is the population of every fed-time statistic below.
    pub fed_moves: usize,
    /// Sum of `length / achieved` over the fed moves (seconds). Read
    /// the module doc's time base — this is not the integrator's cycle
    /// time.
    pub fed_time_s: f64,
    /// Time-weighted mean commanded feed (mm/min). `None` when the
    /// toolpath carries no fed move.
    pub time_weighted_commanded_mm_min: Option<f64>,
    /// Time-weighted mean achieved feed (mm/min) — the total fed
    /// distance divided by the total fed time. `None` when the toolpath
    /// carries no fed move.
    pub time_weighted_achieved_mm_min: Option<f64>,
    /// `time_weighted_achieved_mm_min / time_weighted_commanded_mm_min`
    /// — the utilisation reading. 1.0 means the machine reached every
    /// command. `None` when not measured.
    pub utilization: Option<f64>,
    /// Fraction of fed time by binding. `None` when not measured.
    pub bindings: Option<BindingFractions>,
    /// Fraction of fed time each axis's maximum rate held, `[X, Y, Z]`.
    /// The three sum to [`BindingFractions::rate_bound`]. All three are
    /// zero when the toolpath is not measured, so read
    /// [`Self::is_measured`] first.
    pub rate_bound_axis_time_fraction: [f64; 3],
    /// Plunge-class observation.
    pub plunge: PlungeClassObservation,
    /// Ramp-class observation.
    pub ramp: RampObservation,
    /// Per-move detail, one entry per non-degenerate move, rapids
    /// INCLUDED (a rapid classifies as [`MotionClass::Retract`]). The
    /// seed move 0 and every zero-length move are absent, so
    /// `moves.len()` is at most `moves_total - 1`. The list stays
    /// whole; a display surface caps it.
    ///
    /// **`#[serde(skip)]`.** The list is the INSTRUMENT's working
    /// detail — the tests and the Phase 3 A/B read it in process — and
    /// it is one entry per move, so a large rough carries tens of
    /// thousands. It rides on
    /// [`crate::tool_load::ToolpathLoadVerdict`], which MCP
    /// `get_tool_load_report` serialises whole, so it stays off the
    /// wire. A value that came from JSON therefore has an EMPTY list.
    /// That is not "no moves were measured" — read
    /// [`Self::fed_moves`] for the population, and
    /// [`Self::headroom_at_1_30`] rather than
    /// [`Self::headroom_estimate`], which needs this list.
    #[serde(skip)]
    pub moves: Vec<MoveUtilization>,
    /// [`Self::headroom_estimate`] at `feed_scale = 1.30`, solved ONCE
    /// by [`analyse_toolpath`].
    ///
    /// Every display surface reads this field. `headroom_estimate`
    /// re-solves every move through
    /// [`crate::machine_kinematics::move_kinematics`], so calling it
    /// from a render loop or an MCP handler costs one full solve per
    /// row per frame. It is also the only headroom reading that
    /// survives serialisation, because [`Self::moves`] does not.
    ///
    /// `None` when the toolpath is not measured.
    pub headroom_at_1_30: Option<f64>,
    /// The machine model the analysis ran against. Stored so
    /// [`Self::headroom_estimate`] can re-solve every move.
    pub kinematics: MachineKinematics,
    /// The machine's travel-rate cap (mm/min) the analysis ran against.
    pub max_feed_mm_min: f64,
    /// Whether the analysed move list carried planned or emitted feeds
    /// (Phase 3). [`analyse_toolpath`] always writes
    /// [`FeedsProvenance::Planned`]; the session's producer stamps the
    /// measured value. `#[serde(default)]` so a value that came from an
    /// older wire format reads the conservative answer.
    #[serde(default)]
    pub feeds_provenance: FeedsProvenance,
}

impl ToolpathKinematicUtilization {
    /// True when the toolpath carries at least one fed move. `bindings`
    /// and the two time-weighted means are `Some` exactly when this is
    /// true. `utilization` carries one extra guard: it also needs a
    /// positive commanded feed somewhere in the population.
    pub fn is_measured(&self) -> bool {
        self.fed_moves > 0
    }

    /// True when the toolpath carries at least one plunge-class move. A
    /// `false` means the plunge observation is ABSENT; it does not mean
    /// the plunges are clean.
    pub fn plunge_is_measured(&self) -> bool {
        self.plunge.population > 0
    }

    /// Fraction of fed time saved if every commanded feed rises by
    /// `feed_scale` — the constant-chipload rpm-and-feed rise the
    /// operator asks about.
    ///
    /// The method re-solves each fed move through
    /// [`crate::machine_kinematics::move_kinematics`] with
    /// `commanded × feed_scale`, then compares the two summed times.
    ///
    /// This is a FIRST-ORDER estimate. It holds the junction velocities
    /// at the values the original feeds produced. A real feed rise also
    /// lifts the cornering ceiling, so the true saving is a little
    /// larger. A move that is already accel-bound or rate-bound
    /// contributes nothing, which is the point of the reading. A scaled
    /// command above the machine's `max_feed` saturates there, so the
    /// estimate is bounded by the machine as well as by the geometry.
    /// Returns `None` when the toolpath is not measured, and when
    /// `feed_scale` is not a positive finite number.
    ///
    /// **It reads [`Self::moves`], which is `#[serde(skip)]`.** A
    /// value that came over the wire has an empty list, so this method
    /// returns `None` there however well the toolpath was measured.
    /// It also re-solves every move. A display surface must read
    /// [`Self::headroom_at_1_30`] instead; call this one only for a
    /// scale the field does not carry, on a value built in process.
    pub fn headroom_estimate(&self, feed_scale: f64) -> Option<f64> {
        if !self.is_measured() || !feed_scale.is_finite() || feed_scale <= 0.0 {
            return None;
        }
        let mut base_s = 0.0_f64;
        let mut scaled_s = 0.0_f64;
        for m in &self.moves {
            if m.is_rapid || m.length_mm <= 1e-9 {
                continue;
            }
            if m.achieved_mm_min <= 1e-9 {
                continue;
            }
            base_s += m.length_mm * 60.0 / m.achieved_mm_min;
            let solved = move_kinematics(
                m.length_mm,
                &m.dir,
                m.v_in_mm_min,
                m.v_out_mm_min,
                m.commanded_mm_min * feed_scale,
                &self.kinematics,
                self.max_feed_mm_min,
            );
            if solved.peak_mm_min <= 1e-9 {
                return None;
            }
            scaled_s += m.length_mm * 60.0 / solved.peak_mm_min;
        }
        if base_s <= 1e-12 {
            return None;
        }
        Some(((base_s - scaled_s) / base_s).clamp(-1.0, 1.0))
    }
}

/// One move, prepared the way the runtime integrator prepares it.
struct Digest {
    source_index: usize,
    length: f64,
    dz: f64,
    dir: [f64; 3],
    end_point: [f64; 3],
    /// The raw commanded feed (mm/min); for a rapid, the effective
    /// travel rate.
    commanded_mm_min: f64,
    /// The `max_feed` argument `move_kinematics` receives. For a fed
    /// move this is the machine cap. For a rapid it is the effective
    /// travel rate, which reproduces the integrator's FLOOR — a rapid
    /// is never slowed to the cutting cap.
    feed_ceiling_mm_min: f64,
    /// Cruise ceiling in mm/s — the command, capped by `feed_ceiling`
    /// and by the direction-aware per-axis rate. The junction limiter
    /// reads this, exactly as the integrator does.
    v_ceiling_mm_s: f64,
    accel: f64,
    is_rapid: bool,
    class: MotionClass,
}

/// The worst plunge seen so far, while the walk runs.
struct WorstPlunge {
    ratio: f64,
    move_index: usize,
    position: [f64; 3],
    z_rate_mm_min: f64,
}

/// The steepest ramp seen so far, while the walk runs.
struct WorstRamp {
    deg_from_horizontal: f64,
    move_index: usize,
}

/// Analyse one emitted toolpath against one machine model.
///
/// A pure function: no simulation, no session, no mutation. Call it
/// after generation, on the toolpath the post-processor will emit.
///
/// * `max_feed_mm_min` is the machine's travel-rate cap.
/// * `rapid_feed_mm_min` is the rapid rate. The effective rate is
///   `max(rapid_feed, max_feed)`, which matches the integrator.
/// * `plunge_rate_mm_min` is the OPERATION's plunge rate. Pass `0.0`
///   when the operation has none; every plunge ratio then reads `None`
///   instead of dividing by zero.
pub fn analyse_toolpath(
    toolpath: &Toolpath,
    toolpath_id: ToolpathId,
    kinematics: &MachineKinematics,
    max_feed_mm_min: f64,
    rapid_feed_mm_min: f64,
    plunge_rate_mm_min: f64,
) -> ToolpathKinematicUtilization {
    let moves_total = toolpath.moves.len();
    let rapid_eff_mm_min = rapid_feed_mm_min.max(max_feed_mm_min);

    // Stage 1 — the integrator's digest list: skip the seed move, skip
    // every degenerate move, take an arc on its chord.
    let mut digests: Vec<Digest> = Vec::with_capacity(moves_total);
    for (offset, pair) in toolpath.moves.windows(2).enumerate() {
        let (Some(prev), Some(curr)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        let dx = curr.target.x - prev.target.x;
        let dy = curr.target.y - prev.target.y;
        let dz = curr.target.z - prev.target.z;
        let length = (dx * dx + dy * dy + dz * dz).sqrt();
        if !length.is_finite() || length <= 1e-9 {
            continue;
        }
        let dir = [dx / length, dy / length, dz / length];
        let (commanded_mm_min, feed_ceiling_mm_min, is_rapid) = match curr.move_type.feed_rate() {
            Some(feed) => (feed, max_feed_mm_min, false),
            None => (rapid_eff_mm_min, rapid_eff_mm_min, true),
        };
        // Mirrors `machine_kinematics::cruise_ceiling` exactly.
        let ceiling_cap_mm_s = (feed_ceiling_mm_min / 60.0).max(1e-6);
        let v_cmd_mm_s = (commanded_mm_min / 60.0).max(1e-6).min(ceiling_cap_mm_s);
        let v_ceiling_mm_s = match kinematics.effective_max_rate_mm_min(&dir) {
            Some(rate_mm_min) => {
                let rate_mm_s = rate_mm_min / 60.0;
                if rate_mm_s < v_cmd_mm_s {
                    rate_mm_s
                } else {
                    v_cmd_mm_s
                }
            }
            None => v_cmd_mm_s,
        };
        digests.push(Digest {
            source_index: offset + 1,
            length,
            dz,
            dir,
            end_point: [curr.target.x, curr.target.y, curr.target.z],
            commanded_mm_min,
            feed_ceiling_mm_min,
            v_ceiling_mm_s,
            accel: kinematics.effective_accel(&dir),
            is_rapid,
            class: classify_move(curr.move_type, [dx, dy, dz]),
        });
    }

    // Stage 2 — walk the list pairwise, exactly as the integrator does.
    let n = digests.len();
    let mut moves: Vec<MoveUtilization> = Vec::with_capacity(n);
    let mut fed_moves = 0_usize;
    let mut fed_time_s = 0.0_f64;
    let mut fed_distance_mm = 0.0_f64;
    let mut commanded_time_product = 0.0_f64;
    let mut feed_bound_s = 0.0_f64;
    let mut accel_bound_s = 0.0_f64;
    let mut rate_bound_s = 0.0_f64;
    let mut junction_bound_s = 0.0_f64;
    let mut rate_axis_s = [0.0_f64; 3];
    let mut plunge_population = 0_usize;
    let mut plunge_over_1x = 0_usize;
    let mut plunge_over_2x = 0_usize;
    let mut worst_plunge: Option<WorstPlunge> = None;
    let mut ramp_population = 0_usize;
    let mut worst_ramp: Option<WorstRamp> = None;
    let mut v_in_mm_s = 0.0_f64;

    for (i, d) in digests.iter().enumerate() {
        let v_out_mm_s = match digests.get(i + 1) {
            Some(next) => junction_velocity(
                &d.dir,
                &next.dir,
                d.v_ceiling_mm_s,
                next.v_ceiling_mm_s,
                d.accel.min(next.accel),
                kinematics.junction_deviation_mm,
                kinematics.max_junction_velocity_mm_min,
                d.is_rapid || next.is_rapid,
            ),
            None => 0.0,
        };
        let solved = move_kinematics(
            d.length,
            &d.dir,
            v_in_mm_s * 60.0,
            v_out_mm_s * 60.0,
            d.commanded_mm_min,
            kinematics,
            d.feed_ceiling_mm_min,
        );
        let achieved_mm_min = if solved.peak_mm_min.is_finite() {
            solved.peak_mm_min.max(0.0)
        } else {
            0.0
        };
        let achieved_z_rate_mm_min = if d.dz < 0.0 {
            d.dir[2].abs() * achieved_mm_min
        } else {
            0.0
        };
        let is_plunge = d.class == MotionClass::Plunge;
        let plunge_ratio = if is_plunge && plunge_rate_mm_min > 1e-9 {
            Some(achieved_z_rate_mm_min / plunge_rate_mm_min)
        } else {
            None
        };

        if !d.is_rapid && achieved_mm_min > 1e-9 {
            let t = d.length * 60.0 / achieved_mm_min;
            fed_moves += 1;
            fed_time_s += t;
            fed_distance_mm += d.length;
            commanded_time_product += t * d.commanded_mm_min;
            match solved.binding {
                KinematicBinding::FeedBound => feed_bound_s += t,
                KinematicBinding::AccelBound => accel_bound_s += t,
                KinematicBinding::JunctionBound => junction_bound_s += t,
                KinematicBinding::RateBound { axis } => {
                    rate_bound_s += t;
                    if let Some(slot) = rate_axis_s.get_mut(axis) {
                        *slot += t;
                    }
                }
            }
        }

        match d.class {
            MotionClass::Plunge => {
                plunge_population += 1;
                if let Some(ratio) = plunge_ratio {
                    if ratio > 1.0 {
                        plunge_over_1x += 1;
                    }
                    if ratio > 2.0 {
                        plunge_over_2x += 1;
                    }
                    let worse = match &worst_plunge {
                        Some(current) => ratio > current.ratio,
                        None => true,
                    };
                    if worse {
                        worst_plunge = Some(WorstPlunge {
                            ratio,
                            move_index: d.source_index,
                            position: d.end_point,
                            z_rate_mm_min: achieved_z_rate_mm_min,
                        });
                    }
                }
            }
            MotionClass::Ramp => {
                ramp_population += 1;
                let sine = d.dir[2].abs().clamp(0.0, 1.0);
                let steep_deg = sine.asin().to_degrees();
                let worse = match &worst_ramp {
                    Some(current) => steep_deg > current.deg_from_horizontal,
                    None => true,
                };
                if worse {
                    worst_ramp = Some(WorstRamp {
                        deg_from_horizontal: steep_deg,
                        move_index: d.source_index,
                    });
                }
            }
            MotionClass::Lateral | MotionClass::Retract => {}
        }

        moves.push(MoveUtilization {
            move_index: d.source_index,
            class: d.class,
            length_mm: d.length,
            commanded_mm_min: d.commanded_mm_min,
            achieved_mm_min,
            binding: solved.binding,
            achieved_z_rate_mm_min,
            plunge_ratio,
            dir: d.dir,
            v_in_mm_min: v_in_mm_s * 60.0,
            v_out_mm_min: v_out_mm_s * 60.0,
            is_rapid: d.is_rapid,
        });
        v_in_mm_s = v_out_mm_s;
    }

    // Stage 3 — the aggregates. An empty population stays absent.
    let measured = fed_moves > 0 && fed_time_s > 1e-12;
    let time_weighted_commanded_mm_min = if measured {
        Some(commanded_time_product / fed_time_s)
    } else {
        None
    };
    let time_weighted_achieved_mm_min = if measured {
        Some(fed_distance_mm * 60.0 / fed_time_s)
    } else {
        None
    };
    // Algebraically the ratio of the two means above; the shared
    // `fed_time_s` cancels, so this cannot disagree with them.
    let utilization = if measured && commanded_time_product > 1e-9 {
        Some(fed_distance_mm * 60.0 / commanded_time_product)
    } else {
        None
    };
    let bindings = if measured {
        let feed_bound = feed_bound_s / fed_time_s;
        let accel_bound = accel_bound_s / fed_time_s;
        let rate_bound = rate_bound_s / fed_time_s;
        let junction_bound = junction_bound_s / fed_time_s;
        Some(BindingFractions {
            feed_bound,
            accel_bound,
            rate_bound,
            junction_bound,
            machine_bound: accel_bound + rate_bound + junction_bound,
        })
    } else {
        None
    };
    let rate_bound_axis_time_fraction = if measured {
        [
            rate_axis_s[0] / fed_time_s,
            rate_axis_s[1] / fed_time_s,
            rate_axis_s[2] / fed_time_s,
        ]
    } else {
        [0.0_f64; 3]
    };

    let plunge = PlungeClassObservation {
        population: plunge_population,
        peak_ratio: worst_plunge.as_ref().map(|w| w.ratio),
        over_1x: plunge_over_1x,
        over_2x: plunge_over_2x,
        worst_move_index: worst_plunge.as_ref().map(|w| w.move_index),
        worst_position: worst_plunge.as_ref().map(|w| w.position),
        worst_achieved_z_rate_mm_min: worst_plunge.as_ref().map(|w| w.z_rate_mm_min),
        plunge_rate_mm_min,
    };
    let ramp = RampObservation {
        population: ramp_population,
        steepest_deg_from_horizontal: worst_ramp.as_ref().map(|w| w.deg_from_horizontal),
        worst_move_index: worst_ramp.as_ref().map(|w| w.move_index),
    };

    let mut out = ToolpathKinematicUtilization {
        toolpath_id,
        moves_total,
        fed_moves,
        fed_time_s,
        time_weighted_commanded_mm_min,
        time_weighted_achieved_mm_min,
        utilization,
        bindings,
        rate_bound_axis_time_fraction,
        plunge,
        ramp,
        moves,
        kinematics: *kinematics,
        max_feed_mm_min,
        headroom_at_1_30: None,
        // The pure kernel cannot know. The session's producer stamps it.
        feeds_provenance: FeedsProvenance::Planned,
    };
    // Stage 4 — solve the display headroom ONCE, here. Every surface
    // reads the field; none re-solves the move list per row per frame,
    // and the reading survives serialisation while `moves` does not.
    out.headroom_at_1_30 = out.headroom_estimate(1.30);
    out
}
