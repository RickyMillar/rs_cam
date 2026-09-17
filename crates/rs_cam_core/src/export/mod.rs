//! Output that is not G-code: the SVG/HTML preview, the toolpath ribbon and
//! its colour ramps, the fingerprint diff, the G-code invariant validator and
//! the shared JSON artifact writer.
//!
//! The G-code emitter itself is `gcode`. The validator sits here, beside the
//! other output surfaces, rather than inside the emitter folder.

// Shared JSON-artifact naming and writing; `trace` and `stock` read it.
pub(crate) mod artifact_io;
pub mod fingerprint;
pub mod gcode_validator;
pub mod ribbon;
pub mod viz;
