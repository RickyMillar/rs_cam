//! Machine kinematics model for cycle-time estimation (F-034).
//!
//! The simulator's pre-F-034 `total_runtime_s` is a naive sum of
//! `segment_length / feed_rate` accumulated across every dexel sample.
//! That treats the machine as if it is always at commanded feed —
//! no spool-up, no spool-down through corners, no rapid acceleration.
//!
//! Real machines obey a trapezoidal (with optional jerk-limit smoothing)
//! velocity profile: accelerate from `v_in` up to commanded feed
//! (capped by `max_feed`), cruise at peak feed for the remaining
//! distance, decelerate to the junction velocity entering the next
//! move. On a corner-heavy 3D rough toolpath the difference between
//! the naive sum and an acceleration-aware integrator can run 30–50 %.
//!
//! This module is intentionally minimal — it computes a single
//! per-toolpath time estimate from the `Toolpath` IR. The integrator
//! is deliberately conservative for v1:
//!
//! * Junction velocity uses GRBL's junction-deviation cornering model
//!   (`v = √(accel · R)`, `R = δ·sin(θ/2)/(1−sin(θ/2))`, δ = `$11`), so
//!   the predicted feeds and cycle time match the real GRBL planner;
//!   capped by the smaller commanded feed and optionally clamped by
//!   `MachineKinematics::max_junction_velocity_mm_min`. Straight-through
//!   runs at the commanded feed; direction reversals full-stop.
//! * Arc moves are treated as straight moves of equal arc length
//!   with the commanded feed. The cornering at the endpoints uses
//!   the chord tangent for junction-velocity geometry.
//! * Jerk is ignored unless `jerk_mm_s3` is `Some` — even then it
//!   only smooths the accel ramp by adding a small fixed time
//!   penalty per accel/decel ramp (the asymmetry between trapezoidal
//!   and S-curve profiles is second-order for v1).
//!
//! Refinements (jerk-limited S-curve integrator, junction velocity
//! derived from path curvature, look-ahead planner emulation) can
//! land in follow-up findings once F-034's predictions are
//! ground-truthed against real-machine measurements.
//!
//! The feature is **purely additive**. Presence or absence of
//! `MachineKinematics` IS the flag — when `MachineProfile::kinematics`
//! is `None` (the default for every preset), the entire integrator
//! is bypassed and `total_runtime_s` keeps its pre-F-034 dexel-sample
//! sum. See `planning/feed_modulation_roadmap.md` for the workstream
//! that consumes the kinematics model in F-035 (predicted feed in
//! gates) and F-036 (per-segment feed modulation).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::geo::P3;
use crate::toolpath::{MoveType, Toolpath};

/// Per-machine linear-axis kinematics limits used by the cycle-time
/// integrator.
///
/// Field units match the rest of the crate: `mm/s²`, `mm/s³`,
/// `mm/min` for `max_junction_velocity`. The integrator converts
/// commanded feed rates from `mm/min` to `mm/s` internally so all
/// time arithmetic is in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MachineKinematics {
    /// Linear-axis acceleration limit (mm/s²) — the **isotropic scalar**.
    ///
    /// Sets the slope of the trapezoidal accel ramp. Shapeoko XXL
    /// ships with a stock value of 250 mm/s²; community-tuned builds
    /// land around 500–800 mm/s². Industrial routers run 2000+
    /// mm/s². When `acceleration_xyz_mm_s2` is `Some`, this scalar is
    /// only used as a direction-less fallback (e.g. the synthetic
    /// [`predicted_achieved_feed`] helper).
    pub acceleration_mm_s2: f64,
    /// Optional per-axis acceleration limits `[X, Y, Z]` (mm/s²), e.g.
    /// from GRBL `$120/$121/$122`. When `Some`, the integrator computes
    /// a **direction-aware** path acceleration `min_i(a_i / |dir_i|)`
    /// per move (see [`MachineKinematics::effective_accel`]), so a
    /// Z-dominant plunge is correctly throttled by the slow `$122`
    /// while a planar XY move runs at the X/Y limit. When `None` the
    /// scalar `acceleration_mm_s2` is used isotropically — the
    /// pre-per-axis behaviour, so old project files and every built-in
    /// preset deserialize byte-identically.
    #[serde(default)]
    pub acceleration_xyz_mm_s2: Option<[f64; 3]>,
    /// GRBL junction-deviation (`$11`) in mm — sets how far the virtual
    /// cornering arc may bow from the exact corner. GRBL's stock
    /// default is 0.010 mm and few users change it. Old project files
    /// without this field deserialize to that default.
    #[serde(default = "default_junction_deviation_mm")]
    pub junction_deviation_mm: f64,
    /// Linear-axis jerk limit (mm/s³). When `None` the integrator
    /// treats jerk as infinite (pure trapezoidal accel ramps). When
    /// `Some`, a small fixed time penalty is added per accel/decel
    /// segment to approximate the rounding the jerk limit introduces.
    pub jerk_mm_s3: Option<f64>,
    /// Optional hard cap (mm/min) on junction velocity through any corner.
    /// When `None`, the integrator uses GRBL's junction-deviation model alone
    /// (`v = √(accel · R)`, capped by the smaller commanded feed). When `Some`,
    /// that result is additionally clamped by this constant.
    pub max_junction_velocity_mm_min: Option<f64>,
}

/// GRBL `$11` stock default (mm). Used as the serde default for
/// `MachineKinematics::junction_deviation_mm` on legacy project files.
pub fn default_junction_deviation_mm() -> f64 {
    0.010
}

impl Default for MachineKinematics {
    /// Conservative isotropic wood-router default (200 mm/s², no
    /// per-axis limits, stock junction deviation, no jerk/clamp).
    /// Lets new struct literals spread `..Default::default()` so a
    /// future field add stays one-line at each call site.
    fn default() -> Self {
        Self {
            acceleration_mm_s2: 200.0,
            acceleration_xyz_mm_s2: None,
            junction_deviation_mm: default_junction_deviation_mm(),
            jerk_mm_s3: None,
            max_junction_velocity_mm_min: None,
        }
    }
}

/// Result of parsing a GRBL / grblHAL `$$` settings dump
/// ([`MachineKinematics::from_grbl_settings`]).
///
/// Fields the dump doesn't carry are left `None` / defaulted — the
/// caller (GUI / MCP) decides which to apply onto the live
/// `MachineProfile`. `kinematics` is always populated (absent settings
/// fall back to [`MachineKinematics::default`]).
#[derive(Debug, Clone, PartialEq)]
pub struct GrblImport {
    /// Kinematics built from `$11` (junction deviation) and
    /// `$120/$121/$122` (per-axis acceleration). When all three axis
    /// accels are present they populate `acceleration_xyz_mm_s2` and the
    /// scalar `acceleration_mm_s2` is set to their mean.
    pub kinematics: MachineKinematics,
    /// Travel/max-rate cap (mm/min) for the XY plane — the larger of
    /// `$110`/`$111` — suitable for `MachineProfile::max_feed_mm_min`.
    pub max_feed_mm_min: Option<f64>,
    /// Z max-rate (`$112`, mm/min), surfaced separately (the XY cap
    /// above intentionally ignores the slow Z axis so planar cutting
    /// isn't throttled to the plunge rate).
    pub max_z_feed_mm_min: Option<f64>,
    /// Arc tolerance (`$12`, mm) — feeds the post / arc-fit advice.
    pub arc_tolerance_mm: Option<f64>,
    /// Max spindle RPM (`$30`), if the dump carries it.
    pub max_spindle_rpm: Option<f64>,
    /// Count of `$N` settings seen but not consumed — lets the UI note
    /// "imported N of M settings" without pretending it used them all.
    pub ignored_count: usize,
}

impl MachineKinematics {
    /// Stock Shapeoko XXL kinematics (250 mm/s², no jerk limit, no
    /// junction-velocity clamp). This is the conservative ground
    /// truth most rs_cam users start from; calibration measurements
    /// for F-034 are anchored on a stock-tuned XXL.
    pub fn shapeoko_xxl_stock() -> Self {
        Self {
            acceleration_mm_s2: 250.0,
            ..Self::default()
        }
    }

    /// Conservative "any wood router" defaults — slightly under stock
    /// Shapeoko XXL to be safe for the lowest-rigidity hobby machines
    /// in the wild.
    pub fn generic_wood_router() -> Self {
        Self {
            acceleration_mm_s2: 200.0,
            ..Self::default()
        }
    }

    /// Shapeoko XXL with user-tuned (above-stock) accel + travel rates.
    /// Captured from `$$` on 2026-05-26: `$120 = $121 = 500 mm/s²`
    /// (X/Y), `$122 = 270 mm/s²` (Z), `$110 = $111 = 10000 mm/min`
    /// (X/Y), `$112 = 1000 mm/min` (Z).
    ///
    /// The scalar `acceleration_mm_s2` uses a **blended 350 mm/s²** —
    /// not the pure X/Y or Z value. Empirically calibrated against
    /// wanaka Back Rough wall-clock 827 s (2026-05-26): pure X/Y (500)
    /// over-predicts speed by 22 %; pure Z (270) under-predicts by
    /// ~5 %. 350 lands within ±10 % on the mixed-axis adaptive3d
    /// toolpath. Tuning a per-axis kinematics struct is a F-034
    /// follow-up; until then this scalar absorbs the geometry mix.
    /// The `max_feed_mm_min` cap should be set on the `MachineProfile`
    /// itself to match the user's `$110/$111 = 10000 mm/min`. F4
    /// (2026-06-10): that field is the TRAVEL rate consumed here by
    /// the cycle-time integrator; cutting feeds are bounded separately
    /// by `MachineProfile::cutting_feed_ceiling_mm_min`, so setting
    /// travel to 10000 no longer lets the optimizer propose cutting
    /// hardwood at 10000.
    /// Per-axis accel limits (`$120/$121/$122 = 500/500/270`) are now
    /// representable via `acceleration_xyz_mm_s2`, but this preset keeps
    /// the calibrated **scalar 350** until Phase E re-benches the
    /// per-axis model against a fresh wall-clock — switching to per-axis
    /// shifts the cycle-time prediction and must be validated, not
    /// assumed (see `planning/ACCEL_FRIENDLY_TOOLPATHS_2026-06-20.md`).
    pub fn shapeoko_xxl_ricky_tuned() -> Self {
        Self {
            acceleration_mm_s2: 350.0,
            ..Self::default()
        }
    }

    /// Direction-aware path acceleration (mm/s²) for a **unit** move
    /// direction `dir`.
    ///
    /// With per-axis limits set (`acceleration_xyz_mm_s2 = Some`),
    /// returns the GRBL per-axis cap `min_i(a_i / |dir_i|)`: the path
    /// may accelerate *faster* than any single axis on a diagonal
    /// (both axes share the load) and is throttled on a Z-heavy move by
    /// the slow Z axis. Without per-axis limits, returns the isotropic
    /// scalar `acceleration_mm_s2`. Always ≥ 1e-3 so downstream
    /// division is safe.
    pub fn effective_accel(&self, dir: &[f64; 3]) -> f64 {
        let scalar = self.acceleration_mm_s2.max(1e-3);
        match self.acceleration_xyz_mm_s2 {
            Some(axes) => {
                let mut lim = f64::INFINITY;
                for (a, d) in axes.iter().zip(dir.iter()) {
                    let d_abs = d.abs();
                    if d_abs > 1e-9 {
                        lim = lim.min(a.max(1e-3) / d_abs);
                    }
                }
                if lim.is_finite() {
                    lim.max(1e-3)
                } else {
                    scalar
                }
            }
            None => scalar,
        }
    }

    /// Parse a GRBL / grblHAL `$$` settings dump into kinematics + rate
    /// caps. Accepts the universal `$N=value` line format, tolerating
    /// trailing `(description)` comments (grblHAL verbose form), CRLF,
    /// blank lines, and any unknown `$N`. Lines that don't match
    /// `$<int>=<number>` are skipped, so pasting a whole console session
    /// (with `ok`, banners, etc.) is fine.
    ///
    /// Mapping:
    /// * `$11`  → `junction_deviation_mm`
    /// * `$120/$121/$122` → per-axis `acceleration_xyz_mm_s2` (all three
    ///   required for the array; scalar fallback = their mean. A partial
    ///   set just updates the scalar.)
    /// * `$110/$111` → `max_feed_mm_min` (XY travel; the larger)
    /// * `$112` → `max_z_feed_mm_min`
    /// * `$12`  → `arc_tolerance_mm`
    /// * `$30`  → `max_spindle_rpm`
    ///
    /// Non-GRBL formats (Marlin `M201`, LinuxCNC INI) won't match and
    /// yield an all-default import — the caller should treat an import
    /// that changed nothing as "unrecognised, fall back to manual".
    pub fn from_grbl_settings(dump: &str) -> GrblImport {
        let mut map: BTreeMap<u32, f64> = BTreeMap::new();
        for line in dump.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix('$') else {
                continue;
            };
            let Some((num, val)) = rest.split_once('=') else {
                continue;
            };
            let Ok(n) = num.trim().parse::<u32>() else {
                continue;
            };
            // Value may carry a trailing comment: "0.010 (Junction deviation)"
            // or be directly followed by '(' with no space.
            let val_tok = val.trim();
            let val_tok = val_tok.split_whitespace().next().unwrap_or("");
            let val_tok = val_tok.split('(').next().unwrap_or(val_tok);
            if let Ok(v) = val_tok.parse::<f64>() {
                map.insert(n, v);
            }
        }

        const KNOWN: [u32; 9] = [11, 12, 30, 110, 111, 112, 120, 121, 122];
        let ignored_count = map.keys().filter(|k| !KNOWN.contains(k)).count();

        let mut kinematics = MachineKinematics::default();
        if let Some(&jd) = map.get(&11) {
            kinematics.junction_deviation_mm = jd;
        }
        let (ax, ay, az) = (
            map.get(&120).copied(),
            map.get(&121).copied(),
            map.get(&122).copied(),
        );
        if let (Some(ax), Some(ay), Some(az)) = (ax, ay, az) {
            kinematics.acceleration_xyz_mm_s2 = Some([ax, ay, az]);
            kinematics.acceleration_mm_s2 = (ax + ay + az) / 3.0;
        } else if let Some(a) = ax.or(ay).or(az) {
            // Partial axis set — best effort scalar, no per-axis array.
            kinematics.acceleration_mm_s2 = a;
        }

        let max_feed_mm_min = match (map.get(&110).copied(), map.get(&111).copied()) {
            (Some(x), Some(y)) => Some(x.max(y)),
            (Some(x), None) => Some(x),
            (None, Some(y)) => Some(y),
            (None, None) => None,
        };

        GrblImport {
            kinematics,
            max_feed_mm_min,
            max_z_feed_mm_min: map.get(&112).copied(),
            arc_tolerance_mm: map.get(&12).copied(),
            max_spindle_rpm: map.get(&30).copied(),
            ignored_count,
        }
    }
}

/// Compute the wall-clock cycle time (seconds) the configured machine
/// would take to execute `toolpath`, given commanded feeds in the IR,
/// the machine's `max_feed_mm_min` cap, and the rapid-move feed used
/// for `MoveType::Rapid`.
///
/// The integrator walks the move list pairwise so each segment can
/// see its junction velocity with the previous and next move. For
/// each move the time is the standard trapezoidal-profile integral:
///
/// 1. accelerate from `v_in` toward `v_cmd` at `kinematics.acceleration_mm_s2`,
/// 2. cruise at the peak velocity actually reached,
/// 3. decelerate to `v_out` at the same accel limit.
///
/// If the move is too short to reach `v_cmd` even using all of its
/// length on the accel + decel ramps, the integrator solves for the
/// triangular profile's peak velocity.
///
/// Returns 0.0 for an empty toolpath (no moves to traverse).
pub fn compute_cycle_time(
    toolpath: &Toolpath,
    kinematics: &MachineKinematics,
    max_feed_mm_min: f64,
    rapid_feed_mm_min: f64,
) -> f64 {
    if toolpath.moves.len() < 2 {
        return 0.0;
    }

    // Convert feed rates from mm/min → mm/s once up front.
    let max_feed_mm_s = (max_feed_mm_min / 60.0).max(1e-6);
    let rapid_feed_mm_s = (rapid_feed_mm_min / 60.0).max(max_feed_mm_s);

    // Pre-compute every move's per-move velocity cap (commanded feed
    // capped by machine max feed), direction vector, and the
    // direction-aware path acceleration (per-axis limits when set).
    // Skipping zero-length moves keeps the junction-velocity geometry
    // well-defined.
    struct MoveDigest {
        length: f64,
        dir: [f64; 3],
        v_cmd_mm_s: f64,
        accel: f64,
        is_rapid: bool,
    }

    let mut digests: Vec<MoveDigest> = Vec::with_capacity(toolpath.moves.len());
    #[allow(clippy::indexing_slicing)]
    // SAFETY: bounded by `toolpath.moves.len()`.
    for i in 1..toolpath.moves.len() {
        let p0 = &toolpath.moves[i - 1].target;
        let p1 = &toolpath.moves[i].target;
        let length = chord_length(p0, p1, toolpath.moves[i].move_type);
        if length <= 1e-9 {
            continue;
        }
        let dir = unit_vec(p0, p1);
        let (v_cmd_mm_s, is_rapid) = match toolpath.moves[i].move_type {
            MoveType::Rapid => (rapid_feed_mm_s, true),
            MoveType::Linear { feed_rate }
            | MoveType::ArcCW { feed_rate, .. }
            | MoveType::ArcCCW { feed_rate, .. } => {
                let cmd = (feed_rate / 60.0).max(1e-6).min(max_feed_mm_s);
                (cmd, false)
            }
        };
        let accel = kinematics.effective_accel(&dir);
        digests.push(MoveDigest {
            length,
            dir,
            v_cmd_mm_s,
            accel,
            is_rapid,
        });
    }

    if digests.is_empty() {
        return 0.0;
    }

    // Junction velocity entering move i — first move starts at rest.
    let mut v_in = 0.0;
    let mut total_time_s = 0.0;
    let n = digests.len();
    #[allow(clippy::indexing_slicing)]
    // SAFETY: i bounded by digests.len(); i+1 guarded by `i < n - 1`.
    for i in 0..n {
        let v_cmd = digests[i].v_cmd_mm_s;
        // Junction with the next move. Last move ends at rest.
        let v_out = if i + 1 < n {
            junction_velocity(
                &digests[i].dir,
                &digests[i + 1].dir,
                v_cmd,
                digests[i + 1].v_cmd_mm_s,
                digests[i].accel.min(digests[i + 1].accel),
                kinematics.junction_deviation_mm,
                kinematics.max_junction_velocity_mm_min,
                digests[i].is_rapid || digests[i + 1].is_rapid,
            )
        } else {
            0.0
        };
        let t = trapezoidal_time(digests[i].length, v_in, v_out, v_cmd, digests[i].accel);
        // Jerk penalty: if a jerk limit is configured, the accel and
        // decel ramps each take an additional `accel / jerk` seconds
        // to round their edges. This is a first-order approximation
        // of the S-curve profile and only fires when v_in != v_cmd
        // or v_out != v_cmd (i.e. there's an actual ramp to round).
        let jerk_penalty = if let Some(jerk) = kinematics.jerk_mm_s3 {
            if jerk > 1e-3 {
                let rounding = digests[i].accel / jerk;
                let in_ramp = if (v_cmd - v_in).abs() > 1e-6 {
                    rounding
                } else {
                    0.0
                };
                let out_ramp = if (v_cmd - v_out).abs() > 1e-6 {
                    rounding
                } else {
                    0.0
                };
                in_ramp + out_ramp
            } else {
                0.0
            }
        } else {
            0.0
        };
        total_time_s += t + jerk_penalty;
        v_in = v_out;
    }

    total_time_s
}

/// F-035 — Per-move predicted achieved feed (mm/min) keyed by
/// `(toolpath_id, move_index)`.
///
/// Constructed once per simulation when the
/// `use_predicted_feed_in_gates` flag is on **and** the active
/// `MachineProfile` carries kinematics. The chipload and power gates
/// look up `(sample.toolpath_id, sample.move_index)` to retrieve the
/// realistic feed the machine reached on that move under accel
/// limits, instead of trusting the commanded feed the controller was
/// asked to hit.
///
/// Stored as a single flat `BTreeMap` (toolpath_id, move_index) → feed
/// so a single trace can carry predictions for any mix of toolpaths.
/// Empty / absent → fall back to commanded feed (pre-F-035 behaviour).
pub type PredictedFeedMap = BTreeMap<(crate::ids::ToolpathId, usize), f64>;

/// F-035 — Compute the per-move predicted achieved feed (mm/min) for a
/// single toolpath under the given kinematics limits.
///
/// Walks the same pairwise integrator F-034's [`compute_cycle_time`]
/// uses, but instead of accumulating time, records the *peak velocity*
/// reached on each non-degenerate move. The peak velocity is the
/// trapezoidal/triangular profile's cruise speed — for long moves
/// between two junctions with high junction velocity this equals the
/// commanded feed; for short moves between two tight corners it
/// drops below the commanded feed because the move runs out of
/// distance before the accel ramp reaches `v_cmd`.
///
/// Out:
/// * `feeds_mm_min` — entries keyed by **original** `Toolpath::moves`
///   index. Move 0 (the initial seed `rapid_to`) and zero-length
///   moves are omitted; the gates skip those samples too (rapids are
///   `!is_cutting` and zero-length moves emit no samples).
/// * Returned values are in `mm/min` to match the simulator's
///   `feed_rate_mm_min` units.
///
/// Rapid moves get their commanded `rapid_feed_mm_min` capped by
/// `max_feed_mm_min` and the same accel-aware peak calculation. They
/// don't reach the chipload/power gates (which filter
/// `!is_cutting` samples) but including them in the map keeps the
/// `(toolpath_id, move_index)` indexing consistent with the
/// simulator's sample stream.
pub fn predicted_feeds_for_toolpath(
    toolpath: &Toolpath,
    kinematics: &MachineKinematics,
    max_feed_mm_min: f64,
    rapid_feed_mm_min: f64,
) -> BTreeMap<usize, f64> {
    let mut out = BTreeMap::new();
    if toolpath.moves.len() < 2 {
        return out;
    }

    let max_feed_mm_s = (max_feed_mm_min / 60.0).max(1e-6);
    let rapid_feed_mm_s = (rapid_feed_mm_min / 60.0).max(max_feed_mm_s);

    // Same digest shape as `compute_cycle_time`, plus the original
    // toolpath-move index so callers can key the result map by it.
    struct MoveDigest {
        source_index: usize,
        length: f64,
        dir: [f64; 3],
        v_cmd_mm_s: f64,
        accel: f64,
        is_rapid: bool,
    }

    let mut digests: Vec<MoveDigest> = Vec::with_capacity(toolpath.moves.len());
    #[allow(clippy::indexing_slicing)]
    // SAFETY: bounded by `toolpath.moves.len()`.
    for i in 1..toolpath.moves.len() {
        let p0 = &toolpath.moves[i - 1].target;
        let p1 = &toolpath.moves[i].target;
        let length = chord_length(p0, p1, toolpath.moves[i].move_type);
        if length <= 1e-9 {
            continue;
        }
        let dir = unit_vec(p0, p1);
        let (v_cmd_mm_s, is_rapid) = match toolpath.moves[i].move_type {
            MoveType::Rapid => (rapid_feed_mm_s, true),
            MoveType::Linear { feed_rate }
            | MoveType::ArcCW { feed_rate, .. }
            | MoveType::ArcCCW { feed_rate, .. } => {
                let cmd = (feed_rate / 60.0).max(1e-6).min(max_feed_mm_s);
                (cmd, false)
            }
        };
        let accel = kinematics.effective_accel(&dir);
        digests.push(MoveDigest {
            source_index: i,
            length,
            dir,
            v_cmd_mm_s,
            accel,
            is_rapid,
        });
    }
    if digests.is_empty() {
        return out;
    }

    let mut v_in = 0.0;
    let n = digests.len();
    #[allow(clippy::indexing_slicing)]
    // SAFETY: i bounded by digests.len(); i+1 guarded by `i < n - 1`.
    for i in 0..n {
        let v_cmd = digests[i].v_cmd_mm_s;
        let v_out = if i + 1 < n {
            junction_velocity(
                &digests[i].dir,
                &digests[i + 1].dir,
                v_cmd,
                digests[i + 1].v_cmd_mm_s,
                digests[i].accel.min(digests[i + 1].accel),
                kinematics.junction_deviation_mm,
                kinematics.max_junction_velocity_mm_min,
                digests[i].is_rapid || digests[i + 1].is_rapid,
            )
        } else {
            0.0
        };
        let v_peak_mm_s =
            trapezoidal_peak_velocity(digests[i].length, v_in, v_out, v_cmd, digests[i].accel);
        out.insert(digests[i].source_index, v_peak_mm_s * 60.0);
        v_in = v_out;
    }

    out
}

/// F-035 — single-move predicted achieved feed (mm/min) given the
/// move's commanded feed, distance, and junction velocities with
/// its prev/next neighbours.
///
/// Light-weight wrapper around the same trapezoidal peak-velocity
/// solver [`predicted_feeds_for_toolpath`] uses, exposed for the
/// synthetic-toolpath acceptance tests which build their inputs
/// directly without staging a full `Toolpath`.
pub fn predicted_achieved_feed(
    length_mm: f64,
    v_in_mm_min: f64,
    v_out_mm_min: f64,
    v_cmd_mm_min: f64,
    kinematics: &MachineKinematics,
    max_feed_mm_min: f64,
) -> f64 {
    let accel = kinematics.acceleration_mm_s2.max(1e-3);
    let v_cmd_mm_s = (v_cmd_mm_min / 60.0).max(1e-6).min(max_feed_mm_min / 60.0);
    let v_in_mm_s = (v_in_mm_min / 60.0).max(0.0);
    let v_out_mm_s = (v_out_mm_min / 60.0).max(0.0);
    trapezoidal_peak_velocity(length_mm, v_in_mm_s, v_out_mm_s, v_cmd_mm_s, accel) * 60.0
}

/// Peak velocity reached on a single trapezoidal/triangular profile
/// move. Mirrors the regime split in [`trapezoidal_time`] but
/// returns velocity (mm/s) instead of time.
///
/// * Full trapezoid (`d_accel + d_decel ≤ length`) → returns `v_cmd`,
///   the cruise velocity.
/// * Triangular profile (move too short for full ramps) → returns the
///   smaller peak velocity solved from
///   `v_peak² = a·length + (v_in² + v_out²)/2`, clamped to `v_cmd`.
fn trapezoidal_peak_velocity(length: f64, v_in: f64, v_out: f64, v_cmd: f64, accel: f64) -> f64 {
    if length <= 1e-9 || accel <= 1e-9 {
        return v_cmd.max(0.0);
    }
    let v_in = v_in.max(0.0).min(v_cmd);
    let v_out = v_out.max(0.0).min(v_cmd);
    let d_accel = (v_cmd * v_cmd - v_in * v_in) / (2.0 * accel);
    let d_decel = (v_cmd * v_cmd - v_out * v_out) / (2.0 * accel);
    if d_accel + d_decel <= length {
        v_cmd
    } else {
        let v_peak_sq = accel * length + 0.5 * (v_in * v_in + v_out * v_out);
        v_peak_sq.max(0.0).sqrt().min(v_cmd)
    }
}

/// Chord length between two points. For arcs, the IR doesn't carry
/// enough information for the integrator's purposes — we approximate
/// the arc by its chord. This under-estimates corner-heavy arc paths
/// slightly; future refinement can re-derive the arc length from
/// `i`, `j` and the endpoints when fidelity matters.
fn chord_length(p0: &P3, p1: &P3, _move_type: MoveType) -> f64 {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let dz = p1.z - p0.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Unit direction vector from `p0` to `p1`. Caller has already
/// guaranteed non-zero distance.
fn unit_vec(p0: &P3, p1: &P3) -> [f64; 3] {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let dz = p1.z - p0.z;
    let len = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-12);
    [dx / len, dy / len, dz / len]
}

/// Estimate the junction velocity (mm/s) between two moves using GRBL's
/// **junction-deviation** cornering model — the same one the Shapeoko's GRBL
/// planner runs, so the predicted feeds and cycle time match what the machine
/// actually does (replacing the old dot-product heuristic that full-stopped at
/// every ≥90° corner).
///
/// GRBL fits a virtual arc of radius `R` into the corner that deviates from the
/// exact vertex by at most `JUNCTION_DEVIATION_MM` ($11), then limits the
/// corner speed to the centripetal bound `v = √(accel · R)`:
///
/// ```text
///   cos θ = dir_in · dir_out       (aligned = +1, 90° = 0, reversal = −1)
///   sin(θ/2) = √((1 + cos θ) / 2)
///   R = δ · sin(θ/2) / (1 − sin(θ/2))
///   v_junction = √(accel · R)
/// ```
///
/// * Aligned / straight-through → `R → ∞` → no slowdown (capped by the smaller
///   commanded feed).
/// * 90° turn → `v = √(accel · 2.414 · δ)` (≈ 2.9 mm/s at 350 mm/s², δ=0.01).
/// * Reversal → `R = 0` → full stop.
///
/// `junction_deviation_mm` is the machine's `$11`; `accel` is the corner's
/// path acceleration (the smaller of the two adjacent moves' direction-aware
/// accels). Result is capped by the smaller of the two commanded feeds and any
/// explicit `max_junction_velocity_mm_min`. Rapid-adjacent junctions full-stop,
/// matching the conservative planner convention (clean accel/decel transition).
// Private integrator helper: the corner geometry genuinely needs both
// directions, both commanded feeds, corner accel, δ, the optional clamp,
// and the rapid flag. Bundling them into a struct would only obscure the
// call sites in the two pairwise integrators above.
#[allow(clippy::too_many_arguments)]
fn junction_velocity(
    dir_in: &[f64; 3],
    dir_out: &[f64; 3],
    v_cmd_in: f64,
    v_cmd_out: f64,
    accel: f64,
    junction_deviation_mm: f64,
    max_junction_velocity_mm_min: Option<f64>,
    rapid_adjacent: bool,
) -> f64 {
    if rapid_adjacent {
        return 0.0;
    }
    let cap = v_cmd_in.min(v_cmd_out);
    let apply_clamp = |v: f64| match max_junction_velocity_mm_min {
        Some(limit_mm_min) => v.min(limit_mm_min / 60.0),
        None => v,
    };
    let dot =
        (dir_in[0] * dir_out[0] + dir_in[1] * dir_out[1] + dir_in[2] * dir_out[2]).clamp(-1.0, 1.0);
    let sin_half = (0.5 * (1.0 + dot)).max(0.0).sqrt();
    if sin_half <= 1e-9 {
        return 0.0; // direction reversal → full stop
    }
    if sin_half >= 1.0 - 1e-9 {
        return apply_clamp(cap); // straight-through → no cornering limit
    }
    let r = junction_deviation_mm * sin_half / (1.0 - sin_half);
    apply_clamp((accel * r).sqrt().min(cap))
}

/// Trapezoidal-profile time for a single move of length `length`
/// starting at `v_in`, ending at `v_out`, accelerating toward `v_cmd`
/// at `accel`.
///
/// Returns the wall-clock seconds the segment takes. Handles the
/// three regimes:
///
/// * The move is long enough to reach `v_cmd` — full trapezoid.
/// * The move is too short to reach `v_cmd` even on a triangular
///   accel/decel profile — solve for triangular peak.
/// * `v_in > v_cmd` or `v_out > v_cmd` (caller's responsibility to
///   keep the junction velocities ≤ both commanded feeds; we clamp
///   defensively).
fn trapezoidal_time(length: f64, v_in: f64, v_out: f64, v_cmd: f64, accel: f64) -> f64 {
    if length <= 1e-9 || accel <= 1e-9 {
        return 0.0;
    }
    let v_in = v_in.max(0.0).min(v_cmd);
    let v_out = v_out.max(0.0).min(v_cmd);

    // Distance to accel from v_in → v_cmd and decel from v_cmd → v_out.
    let d_accel = (v_cmd * v_cmd - v_in * v_in) / (2.0 * accel);
    let d_decel = (v_cmd * v_cmd - v_out * v_out) / (2.0 * accel);

    if d_accel + d_decel <= length {
        // Full trapezoid: accel ramp + cruise + decel ramp.
        let t_accel = (v_cmd - v_in) / accel;
        let t_decel = (v_cmd - v_out) / accel;
        let d_cruise = length - d_accel - d_decel;
        let t_cruise = if v_cmd > 1e-9 { d_cruise / v_cmd } else { 0.0 };
        t_accel + t_cruise + t_decel
    } else {
        // Triangular profile: solve for peak velocity v_peak such that
        //   (v_peak² - v_in²)/(2a) + (v_peak² - v_out²)/(2a) = length
        //   v_peak² = a·length + (v_in² + v_out²)/2
        let v_peak_sq = accel * length + 0.5 * (v_in * v_in + v_out * v_out);
        let v_peak = v_peak_sq.max(0.0).sqrt().min(v_cmd);
        let t_accel = (v_peak - v_in).max(0.0) / accel;
        let t_decel = (v_peak - v_out).max(0.0) / accel;
        t_accel + t_decel
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::toolpath::Toolpath;

    fn shapeoko() -> MachineKinematics {
        MachineKinematics::shapeoko_xxl_stock()
    }

    /// Phase 4: junction velocity follows GRBL's junction-deviation closed form.
    #[test]
    fn junction_velocity_matches_grbl_deviation_closed_form() {
        let accel = 350.0;
        let delta = default_junction_deviation_mm();
        let big = 1e9; // commanded feeds high → corner geometry binds, not the cap
        let x = [1.0, 0.0, 0.0];
        let y = [0.0, 1.0, 0.0];
        let neg_x = [-1.0, 0.0, 0.0];

        // Straight-through: no cornering limit → capped by the commanded feed.
        let straight = junction_velocity(&x, &x, 50.0, 50.0, accel, delta, None, false);
        assert!(
            (straight - 50.0).abs() < 1e-9,
            "straight = cap, got {straight}"
        );

        // 90° turn: sin(45°)=√0.5, R = δ·s/(1−s), v = √(accel·R).
        let s = 0.5_f64.sqrt();
        let expected_90 = (accel * (delta * s / (1.0 - s))).sqrt();
        let got_90 = junction_velocity(&x, &y, big, big, accel, delta, None, false);
        assert!(
            (got_90 - expected_90).abs() < 1e-6,
            "90°: expected {expected_90}, got {got_90}"
        );

        // Reversal → full stop.
        let rev = junction_velocity(&x, &neg_x, big, big, accel, delta, None, false);
        assert!(rev.abs() < 1e-9, "reversal = 0, got {rev}");

        // A shallow (10°) turn must corner faster than a 90° turn.
        let ten = 10.0_f64.to_radians();
        let shallow = [ten.cos(), ten.sin(), 0.0];
        let v_shallow = junction_velocity(&x, &shallow, big, big, accel, delta, None, false);
        assert!(
            v_shallow > got_90,
            "shallow turn must corner faster than 90°: {v_shallow} vs {got_90}"
        );

        // The smaller commanded feed caps the geometric limit when lower.
        let capped = junction_velocity(&x, &y, 0.5, 0.5, accel, delta, None, false);
        assert!(
            (capped - 0.5).abs() < 1e-9,
            "commanded-feed cap should bind, got {capped}"
        );

        // Rapid-adjacent junctions full-stop.
        assert_eq!(
            junction_velocity(&x, &x, big, big, accel, delta, None, true),
            0.0
        );
    }

    #[test]
    fn empty_toolpath_is_zero() {
        let tp = Toolpath::new();
        assert_eq!(compute_cycle_time(&tp, &shapeoko(), 4000.0, 5000.0), 0.0);
    }

    #[test]
    fn single_long_straight_line_matches_naive_within_accel_overhead() {
        // 100 mm at 3000 mm/min = 50 mm/s → naive 2.0 s.
        // With 250 mm/s² accel from rest to 50 mm/s, ramp takes 0.2 s
        // and covers 5 mm; same for decel. Cruise distance 90 mm at
        // 50 mm/s → 1.8 s. Total ≈ 2.2 s. So kinematic > naive by the
        // accel overhead — that's the whole point of F-034.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to(P3::new(100.0, 0.0, 0.0), 3000.0);
        let kinematic_s = compute_cycle_time(&tp, &shapeoko(), 4000.0, 5000.0);
        let naive_s = 100.0 / (3000.0 / 60.0);
        assert!(
            kinematic_s >= naive_s - 1e-6,
            "kinematic {kinematic_s} should be ≥ naive {naive_s}"
        );
        // Accel overhead for one move = (v_cmd / accel) seconds
        // contributed by the asymmetry between ramp + cruise + ramp
        // and pure-cruise. Worst case: 50 / 250 = 0.2 s. Allow a
        // generous bound.
        assert!(
            kinematic_s <= naive_s + 0.3,
            "kinematic {kinematic_s} exceeded naive {naive_s} by too much"
        );
    }

    #[test]
    fn back_to_back_reversal_full_stops_between_moves() {
        // Two moves in opposite directions: planner must full-stop
        // between them. Kinematic time should noticeably exceed
        // naive distance/feed.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to(P3::new(20.0, 0.0, 0.0), 3000.0);
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 3000.0);
        let kinematic_s = compute_cycle_time(&tp, &shapeoko(), 4000.0, 5000.0);
        let naive_s = 40.0 / 50.0; // 0.8 s
        assert!(
            kinematic_s > naive_s * 1.2,
            "reversal should be at least 20 % slower than naive ({kinematic_s} vs {naive_s})"
        );
    }

    #[test]
    fn many_short_zigzag_moves_significantly_exceed_naive() {
        // 20 short reversals — corner-heavy synthetic case.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        let mut sign = 1.0;
        let mut total_dist = 0.0;
        for i in 1..=20 {
            let y = if sign > 0.0 { 1.0 } else { 0.0 };
            sign = -sign;
            tp.feed_to(P3::new(i as f64, y, 0.0), 3000.0);
            total_dist += 1.0_f64.hypot(if i == 1 { 0.0 } else { 1.0 });
        }
        let kinematic_s = compute_cycle_time(&tp, &shapeoko(), 4000.0, 5000.0);
        let naive_s = total_dist / 50.0;
        assert!(
            kinematic_s > naive_s * 1.3,
            "20-corner zigzag should be ≥ 30 % slower than naive ({kinematic_s} vs {naive_s})"
        );
    }

    #[test]
    fn triangular_profile_when_move_too_short() {
        // 1 mm at 3000 mm/min: ramps to 50 mm/s need 5 mm each. The
        // segment can't reach commanded feed; integrator must use the
        // triangular fallback. Peak velocity solves v² = a·length →
        // v_peak = sqrt(250·1) = 15.81 mm/s. Time = 2 · 15.81 / 250
        // ≈ 0.127 s. Naive: 1/50 = 0.02 s.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to(P3::new(1.0, 0.0, 0.0), 3000.0);
        let t = compute_cycle_time(&tp, &shapeoko(), 4000.0, 5000.0);
        assert!(
            (t - 0.1265).abs() < 0.01,
            "triangular profile time should be ~0.127 s, got {t}"
        );
    }

    // ---- F-035 — predicted-feed integrator unit tests --------------

    #[test]
    fn predicted_feed_long_move_reaches_commanded() {
        // 200 mm at 3000 mm/min — plenty of distance for the accel
        // ramp (5 mm) and decel ramp (5 mm) to reach the commanded
        // 50 mm/s cruise. Predicted feed should equal commanded.
        let kin = shapeoko();
        let pf = predicted_achieved_feed(200.0, 0.0, 0.0, 3000.0, &kin, 4000.0);
        assert!(
            (pf - 3000.0).abs() < 1e-6,
            "long move should hit commanded feed exactly, got {pf}"
        );
    }

    #[test]
    fn predicted_feed_short_corner_move_below_commanded() {
        // 1 mm between two full-stop junctions at 3000 mm/min — the
        // machine can't reach 50 mm/s in 0.5 mm of accel. Triangular
        // profile: v_peak = sqrt(a·length) = sqrt(250) ≈ 15.81 mm/s
        // → 949 mm/min, well below commanded 3000.
        let kin = shapeoko();
        let pf = predicted_achieved_feed(1.0, 0.0, 0.0, 3000.0, &kin, 4000.0);
        assert!(
            pf < 3000.0,
            "short move between corners should drop below commanded, got {pf}"
        );
        let expected_mm_s = (250.0_f64 * 1.0).sqrt();
        let expected_mm_min = expected_mm_s * 60.0;
        assert!(
            (pf - expected_mm_min).abs() < 1.0,
            "triangular peak should be {expected_mm_min:.1} mm/min, got {pf:.1}"
        );
    }

    #[test]
    fn predicted_feeds_for_toolpath_keys_match_move_indices() {
        // Two moves: an initial rapid + one long feed. The map should
        // carry exactly one entry, keyed by move-index 1 (the feed),
        // with predicted ≈ commanded.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to(P3::new(200.0, 0.0, 0.0), 3000.0);
        let map = predicted_feeds_for_toolpath(&tp, &shapeoko(), 4000.0, 5000.0);
        assert_eq!(map.len(), 1, "expected one entry, got {map:?}");
        let pf = map.get(&1).copied().expect("move-index 1 should be set");
        assert!(
            (pf - 3000.0).abs() < 1e-6,
            "long feed move should hit commanded 3000 mm/min, got {pf}"
        );
    }

    #[test]
    fn predicted_feeds_for_toolpath_corner_drops_feed() {
        // Two short reversal moves at 3000 mm/min. Junction is full-
        // stop (direction reversal), so both feeds should be well
        // below commanded.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to(P3::new(1.0, 0.0, 0.0), 3000.0);
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 3000.0);
        let map = predicted_feeds_for_toolpath(&tp, &shapeoko(), 4000.0, 5000.0);
        assert_eq!(map.len(), 2);
        for (k, v) in &map {
            assert!(
                *v < 3000.0,
                "move {k} feed should drop below commanded between corner+endpoint, got {v}"
            );
        }
    }

    #[test]
    fn jerk_penalty_adds_time_proportional_to_ramp_count() {
        // Same toolpath; one with jerk = None (pure trapezoid), one
        // with jerk = Some(small). The jerk-penalty version must take
        // strictly longer.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to(P3::new(20.0, 0.0, 0.0), 3000.0);
        tp.feed_to(P3::new(0.0, 0.0, 0.0), 3000.0);
        let pure = compute_cycle_time(&tp, &shapeoko(), 4000.0, 5000.0);
        let smooth_kin = MachineKinematics {
            jerk_mm_s3: Some(500.0),
            ..shapeoko()
        };
        let smooth = compute_cycle_time(&tp, &smooth_kin, 4000.0, 5000.0);
        assert!(
            smooth > pure,
            "jerk penalty should increase total time ({smooth} vs {pure})"
        );
    }

    // ---- per-axis acceleration -------------------------------------

    #[test]
    fn effective_accel_falls_back_to_scalar_without_per_axis() {
        let kin = MachineKinematics {
            acceleration_mm_s2: 350.0,
            ..MachineKinematics::default()
        };
        // Any direction → the scalar, isotropically.
        assert!((kin.effective_accel(&[1.0, 0.0, 0.0]) - 350.0).abs() < 1e-9);
        assert!((kin.effective_accel(&[0.0, 0.0, 1.0]) - 350.0).abs() < 1e-9);
    }

    #[test]
    fn effective_accel_per_axis_limits_by_governing_axis() {
        let kin = MachineKinematics {
            acceleration_xyz_mm_s2: Some([500.0, 500.0, 270.0]),
            ..MachineKinematics::default()
        };
        // Pure-axis moves return that axis' limit exactly.
        assert!((kin.effective_accel(&[1.0, 0.0, 0.0]) - 500.0).abs() < 1e-9);
        assert!((kin.effective_accel(&[0.0, 0.0, 1.0]) - 270.0).abs() < 1e-9);
        // A 45° XY diagonal accelerates *faster* than either axis alone
        // (both axes share the load): 500 / sin45 ≈ 707.
        let diag = kin.effective_accel(&[0.5_f64.sqrt(), 0.5_f64.sqrt(), 0.0]);
        assert!(
            (diag - 500.0 / 0.5_f64.sqrt()).abs() < 1e-6,
            "diagonal accel should be 500/sin45 ≈ 707, got {diag}"
        );
        // A move with any Z component is throttled toward the slow Z axis.
        let z_heavy = kin.effective_accel(&[0.1, 0.0, (1.0_f64 - 0.01).sqrt()]);
        assert!(
            z_heavy < 300.0,
            "Z-dominant move should be limited near $122=270, got {z_heavy}"
        );
    }

    #[test]
    fn per_axis_z_plunge_slower_than_xy_cut_same_geometry() {
        // Identical-length move, one in X (fast axis) one in Z (slow
        // axis): the per-axis model must make the Z move take longer.
        let kin = MachineKinematics {
            acceleration_xyz_mm_s2: Some([500.0, 500.0, 270.0]),
            ..MachineKinematics::default()
        };
        let mut xy = Toolpath::new();
        xy.rapid_to(P3::new(0.0, 0.0, 0.0));
        xy.feed_to(P3::new(5.0, 0.0, 0.0), 3000.0);
        let mut z = Toolpath::new();
        z.rapid_to(P3::new(0.0, 0.0, 0.0));
        z.feed_to(P3::new(0.0, 0.0, -5.0), 3000.0);
        let t_xy = compute_cycle_time(&xy, &kin, 6000.0, 6000.0);
        let t_z = compute_cycle_time(&z, &kin, 6000.0, 6000.0);
        assert!(
            t_z > t_xy,
            "Z plunge (accel 270) should be slower than equal X cut (accel 500): {t_z} vs {t_xy}"
        );
    }

    // ---- GRBL $$ parser --------------------------------------------

    #[test]
    fn from_grbl_settings_maps_known_settings() {
        // A realistic (trimmed) Shapeoko XXL dump, grblHAL verbose form,
        // CRLF line endings, with a stray console line and unknown keys.
        let dump = "ok\r\n$11=0.020 (Junction deviation, mm)\r\n$12=0.002\r\n\
             $30=24000\r\n$110=10000.000\r\n$111=10000.000\r\n$112=1000.000\r\n\
             $120=500.000\r\n$121=500.000\r\n$122=270.000\r\n$100=40.000\r\n";
        let imp = MachineKinematics::from_grbl_settings(dump);
        assert!((imp.kinematics.junction_deviation_mm - 0.020).abs() < 1e-9);
        assert_eq!(
            imp.kinematics.acceleration_xyz_mm_s2,
            Some([500.0, 500.0, 270.0])
        );
        // Scalar fallback = mean of the three axis accels.
        assert!((imp.kinematics.acceleration_mm_s2 - (500.0 + 500.0 + 270.0) / 3.0).abs() < 1e-9);
        assert_eq!(imp.max_feed_mm_min, Some(10000.0));
        assert_eq!(imp.max_z_feed_mm_min, Some(1000.0));
        assert_eq!(imp.arc_tolerance_mm, Some(0.002));
        assert_eq!(imp.max_spindle_rpm, Some(24000.0));
        // $100 (steps/mm) is seen but not consumed.
        assert_eq!(imp.ignored_count, 1);
    }

    // ---- serde IO contract (project file + machine library) --------

    #[test]
    fn kinematics_json_round_trips_with_per_axis() {
        let kin = MachineKinematics {
            acceleration_mm_s2: 423.3,
            acceleration_xyz_mm_s2: Some([500.0, 500.0, 270.0]),
            junction_deviation_mm: 0.02,
            jerk_mm_s3: Some(1000.0),
            max_junction_velocity_mm_min: None,
        };
        let json = serde_json::to_string(&kin).expect("serialize");
        let back: MachineKinematics = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(kin, back);
    }

    #[test]
    fn legacy_kinematics_json_defaults_new_fields() {
        // A pre-per-axis project file / library entry carries only the
        // three original fields. The serde defaults must fill the rest
        // so old files load with byte-identical behaviour.
        let legacy =
            r#"{"acceleration_mm_s2":250.0,"jerk_mm_s3":null,"max_junction_velocity_mm_min":null}"#;
        let kin: MachineKinematics = serde_json::from_str(legacy).expect("legacy deserialize");
        assert_eq!(kin.acceleration_xyz_mm_s2, None);
        assert!((kin.junction_deviation_mm - 0.010).abs() < 1e-12);
        assert!((kin.acceleration_mm_s2 - 250.0).abs() < 1e-12);
    }

    #[test]
    fn from_grbl_settings_partial_and_garbage_are_tolerated() {
        // Only $11 present; no axis accels → no per-axis array, default
        // scalar retained; everything else None.
        let imp = MachineKinematics::from_grbl_settings("garbage\n$11=0.015\nM201 X9\n");
        assert!((imp.kinematics.junction_deviation_mm - 0.015).abs() < 1e-9);
        assert_eq!(imp.kinematics.acceleration_xyz_mm_s2, None);
        assert_eq!(imp.max_feed_mm_min, None);
        assert_eq!(imp.arc_tolerance_mm, None);
    }
}
