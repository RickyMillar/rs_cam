//! The records that describe a generated toolpath: the debug trace, the
//! semantic trace, the spans, the narration and the transform provenance
//! contract.
//!
//! The follow-path OPERATION is `ops::trace_path`, not this folder.

pub mod debug_trace;
pub mod narrate;
pub mod semantic_trace;
pub mod toolpath_spans;
pub mod transform_provenance;
