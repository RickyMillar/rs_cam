//! Metrology — the finishing programme's shared rulers.
//!
//! Track M (`planning/metrology_2026-09-02/TRACK.md`) created this module.
//! Before it, each evidence instrument carried its own copy of each ruler:
//! the costing harness existed in six test files, the floor integrand in
//! three, the spacing measurement in two, and the Monge estimator with its
//! censuses in three. The Track G ledger could not put its arms on one
//! scale, and G-UNIONCOV (`planning/finishing_status_2026-09-01.md` §12)
//! exists because no whole-board coverage ruler existed at all.
//!
//! One implementation of each measurement lives here. Instruments are thin
//! consumers. A promoted function keeps the behavior of the instrument copy
//! it came from; a disclosed divergence between copies becomes an explicit
//! parameter, never a silent unification.
//!
//! # The measurement contract
//!
//! Every consumer of a metrology number must know these four rules.
//!
//! ## 1. `None` is "not measured"; `0.0` is "measured and clean"
//!
//! The `ToolpathStats` finding contract (see `crate::compute::config`)
//! applies here too. Never coerce an absent value to zero. A report field
//! that cannot distinguish the two states must say so in its own doc.
//!
//! ## 2. Two air-cut denominators exist
//!
//! `air_cut_pct_of_total_runtime` divides air-cut time by cutting time plus
//! rapids. Every shipped threshold is tuned against it.
//! `air_cut_pct_of_cutting_time` excludes rapids and always reads higher.
//! A table must name the denominator it uses. See
//! `crate::session` and `crate::simulation_cut`.
//!
//! ## 3. Two time scales exist, and cross-scale tables must label each row
//!
//! * **The session integrator** (`crate::compute` / the F-034 pipeline)
//!   prices a whole simulated project: stored motion, machine kinematics,
//!   per-toolpath runtimes.
//! * **The metrology costing harness** ([`costing`]) prices a single
//!   candidate toolpath after the production relink, with test-pinned feeds
//!   and kinematics.
//!
//! The two agree on the integrator arithmetic but not on scope: the harness
//! never sees setup changes, tool changes, or drill dwell. A table that
//! mixes the two scales must label the scale of every row; only large
//! cross-scale gaps are decided (the Track G lesson,
//! `planning/ledger_2026-09-01/FINDINGS.md`).
//!
//! ## 4. Some quantities are resolution-conditional
//!
//! A grid-sampled quantity moves with its cell size. `rapid_collision_count`
//! is cell-scaled; engagement readings below the grid's floor are hard
//! zeros; the union-coverage audit ([`union_coverage`]) reports areas on its
//! own sample grid and carries that cell size in the report. Never compare
//! two resolution-conditional readings taken at different resolutions.

pub mod census;
pub mod costing;
pub mod floor;
pub mod monge;
pub mod spacing;
pub mod union_coverage;
