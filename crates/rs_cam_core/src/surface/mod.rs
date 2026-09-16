//! What the model surface looks like at a point: the two cutter walks, the
//! slope and rest fields, the flow routing and the reach policy.
//!
//! Every module here depends downward only. A surface analyser that reads a
//! finishing module lives in `finish/` instead.

pub mod dropcutter;
pub mod flow_accum;
pub mod pushcutter;
pub mod reach;
pub mod rest_field;
pub mod slope;
