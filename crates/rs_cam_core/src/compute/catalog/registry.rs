//! The operation registry data: one `*_PARAMS` const and one `REG_*`
//! static per operation.
//!
//! Split out of `compute/catalog.rs` (P4). Pure data — the parent's
//! `OperationType::registry_entry` is the only reader of the `REG_*`
//! statics, so they carry `pub(super)`.

use crate::feeds::support::{DRILL_FORMULA_SOURCE, MILLING_FORMULA_SOURCE};
use crate::feeds::{CutterKind, OperationFamily as FeedsOperationFamily, PassRole};

use super::schema::{
    DressupPolicy, Kinematics, OpPolicy, OpRegistryEntry, ParamDef, ParamRange, ToolConstraintsDef,
};
use super::{
    GeometryRequirement, OperationFamily, OperationSpec, OperationType, UiOperationFamily,
    UiProcessRole,
};

const FACE_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("depth_per_pass", "f64").with_help(
        "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter \
         for large.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_offset", "f64")
        .with_help("Extra distance beyond stock boundary to ensure full coverage."),
    ParamDef::required("direction", "enum:one_way|zigzag"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const POCKET_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("depth_per_pass", "f64").with_help(
        "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter \
         for large.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("climb", "bool"),
    ParamDef::required("pattern", "enum:contour|zigzag"),
    ParamDef::required("angle", "f64")
        .with_help("Zigzag/raster angle in degrees. 0 = along X axis."),
    ParamDef::required("finishing_passes", "usize"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const PROFILE_PARAMS: &[ParamDef] = &[
    // G-SCHEMAENUM: `on` was never a `ProfileSide` and the generator has no
    // on-the-line arm. The on-the-line cut ships twice under its own names —
    // `project_curve` with `side: center` (labelled "On Line") and `trace`
    // with `compensation: none`, whose tool centre follows the path exactly.
    ParamDef::required("side", "enum:inside|outside"),
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("depth_per_pass", "f64").with_help(
        "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter \
         for large.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("climb", "bool"),
    ParamDef::required("tab_count", "usize"),
    ParamDef::required("tab_width", "f64")
        .with_help("Width of holding tabs that keep the part attached to stock."),
    ParamDef::required("tab_height", "f64")
        .with_help("Height of holding tabs from the floor of the cut."),
    ParamDef::required("finishing_passes", "usize"),
    ParamDef::required("compensation", "enum:in_computer|in_control"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const ADAPTIVE_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("depth_per_pass", "f64").with_help(
        "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter \
         for large.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("tolerance", "f64").with_help(
        "Geometric tolerance for path approximation. Smaller = more accurate, \
         slower.",
    ),
    ParamDef::required("slot_clearing", "bool"),
    ParamDef::required("min_cutting_radius", "f64")
        .with_help("Blend sharp corners with arcs of at least this radius."),
    ParamDef::optional("spindle_rpm", "option<u32>"),
    ParamDef::required(
        "cleanup_strategy",
        "enum:Legacy|ResidueMop|ContourParallelNarrow|ContourParallelHybrid",
    ),
    // F1 (algorithm review 2026-06-12): which engagement quantity the
    // direction search compares against the α/2π target.
    ParamDef::required("engagement_measure", "enum:DiskArea|LeadingArc"),
    // Stage 1: reactive agent vs constructive contour spiral.
    ParamDef::required("path_strategy", "enum:Agent|ContourSpiral"),
];

const VCARVE_PARAMS: &[ParamDef] = &[
    ParamDef::required("max_depth", "f64")
        .with_help("Maximum V-carve plunge depth. Limits how deep the V-bit goes."),
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("tolerance", "f64").with_help(
        "Geometric tolerance for path approximation. Smaller = more accurate, \
         slower.",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const REST_PARAMS: &[ParamDef] = &[
    ParamDef::optional_desc(
        "prev_tool_id",
        "option<usize>",
        "Index of the prior (typically larger) tool used to define rest geometry",
    ),
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("depth_per_pass", "f64").with_help(
        "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter \
         for large.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("angle", "f64")
        .with_help("Zigzag/raster angle in degrees. 0 = along X axis."),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const INLAY_PARAMS: &[ParamDef] = &[
    ParamDef::required("pocket_depth", "f64")
        .with_help("Depth of the inlay pocket measured from stock surface."),
    ParamDef::required("glue_gap", "f64")
        .with_help("Gap between male/female inlay pieces for glue. 0.05-0.15mm."),
    ParamDef::required("flat_depth", "f64")
        .with_help("Depth for flat-bottom clearing in the inlay pocket. 0 = V-only."),
    ParamDef::required("boundary_offset", "f64")
        .with_help("Offset from the design boundary for the inlay cut. Adjusts fit."),
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("flat_tool_radius", "f64")
        .with_help("Radius of the flat endmill used to clear the pocket floor."),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("tolerance", "f64").with_help(
        "Geometric tolerance for path approximation. Smaller = more accurate, \
         slower.",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const ZIGZAG_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("depth_per_pass", "f64").with_help(
        "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter \
         for large.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("angle", "f64")
        .with_help("Zigzag/raster angle in degrees. 0 = along X axis."),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const TRACE_PARAMS: &[ParamDef] = &[
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("depth_per_pass", "f64").with_help(
        "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter \
         for large.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    // G-SCHEMAENUM: the variant is `none`, not `center`.
    ParamDef::required("compensation", "enum:none|left|right"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const DRILL_PARAMS: &[ParamDef] = &[
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("cycle", "enum:simple|dwell|peck|chip_break"),
    ParamDef::required_ranged(
        "peck_depth",
        "f64",
        ParamRange::greater_than(0.0),
        "Depth of each peck (mm), rooted at the R-plane like Fanuc G83. \
         Must be strictly positive and finite: `drill::fed_descents` \
         degrades a zero, negative or non-finite peck to a single \
         full-depth descent, so any such value silently turns a Peck \
         cycle into a single-shot one (DR-LIVE).",
    ),
    ParamDef::required("dwell_time", "f64"),
    ParamDef::required("retract_amount", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("retract_z", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const CHAMFER_PARAMS: &[ParamDef] = &[
    ParamDef::required("chamfer_width", "f64").with_help(
        "Width of the chamfer on the face (mm). Depth computed from tool \
         angle.",
    ),
    ParamDef::required("tip_offset", "f64").with_help(
        "Distance from V-bit tip to prevent wear. Increases cut depth \
         slightly.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const DROP_CUTTER_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("min_z", "f64")
        .with_help("Lowest Z the tool will descend to during drop-cutter."),
    ParamDef::required("slope_from", "f64").with_help(
        "Minimum surface slope (degrees) to machine. Faces shallower than \
         this are skipped.",
    ),
    ParamDef::required("slope_to", "f64").with_help(
        "Maximum surface slope (degrees) to machine. Steeper faces are \
         skipped.",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
    // CMP-09: the Suggest pipeline reads this and the GUI has a dedicated
    // route to it (`set_drop_cutter_scallop_height`), so an operator could
    // turn the dial and an agent could not. `None` keeps the legacy
    // formula stepover.
    ParamDef::optional_desc(
        "scallop_height",
        "option<f64>",
        "Target scallop (cusp) height in mm. When set, Suggest derives stepover from the \
         tool's tip radius instead of the ae_factor formula. No effect on a flat or bull tool.",
    ),
    // G-LINKSTAGE (2026-09-09) — the shared surface-link stage's cap.
    // Absent, or 0.0, is OFF and byte-identical.
    ParamDef::optional("hookup_mm", "f64"),
];

const ADAPTIVE3D_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("depth_per_pass", "f64").with_help(
        "Max depth per Z level. Wood: 1-3mm small tools, up to half diameter \
         for large.",
    ),
    ParamDef::required("stock_to_leave_axial", "f64").with_help(
        "Finishing allowance kept on the surface for a later pass. Applied as \
         a vertical offset: on a wall sloped at angle A, what remains \
         measured normal to the surface is this value x cos(A).",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("tolerance", "f64").with_help(
        "Geometric tolerance for path approximation. Smaller = more accurate, \
         slower.",
    ),
    ParamDef::required("min_cutting_radius", "f64")
        .with_help("Blend sharp corners with arcs of at least this radius."),
    ParamDef::required("entry_style", "enum:plunge|helix|ramp"),
    ParamDef::required("ramp_angle_deg", "f64").with_help(
        "Ramp entry angle from horizontal (degrees), for the Ramp entry \
         style.",
    ),
    ParamDef::required("helix_radius_factor", "f64")
        .with_help("Helix entry radius as a multiple of the tool diameter."),
    ParamDef::required("helix_pitch", "f64")
        .with_help("Vertical drop per revolution of the helical entry move."),
    ParamDef::required("fine_stepdown", "f64")
        .with_help("Optional finer Z step for final passes. 0 = disabled."),
    ParamDef::required("detect_flat_areas", "bool"),
    ParamDef::required("region_ordering", "enum:global|by_area"),
    ParamDef::required(
        "clearing_strategy",
        "enum:contour_parallel|adaptive|agent_search|contour_spiral",
    ),
    // "Nibble" dial — trochoid trigger cap for the ContourSpiral strategy
    // (low = flat load/more travel, high = relaxed/less travel). Default
    // 1.6. Ignored by the other strategies.
    ParamDef::required("trochoid_cap_mult", "f64"),
    // F1 (algorithm review 2026-06-12): engagement quantity for the
    // AgentSearch 2D sub-pass.
    ParamDef::required("engagement_measure", "enum:DiskArea|LeadingArc"),
    ParamDef::required("z_blend", "bool"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
    ParamDef::required("mill_shallow_areas", "bool"),
    ParamDef::optional("shallow_angle_deg", "option<f64>"),
    ParamDef::optional("shallow_stepdown", "option<f64>"),
    ParamDef::required("min_region_cut_length_mm", "f64"),
    // F-038b: keep-tool-down link knobs.
    ParamDef::optional("max_stay_down_distance_mm", "option<f64>"),
    ParamDef::required("stay_down_clearance_mm", "f64"),
];

const WATERLINE_PARAMS: &[ParamDef] = &[
    // CMP-08: `depth_per_pass` is the alias `set_toolpath_param`'s named
    // arm writes onto this field. It is published here, so the schema and
    // the refusal message agree with the setter.
    ParamDef::required("z_step", "f64")
        .with_aliases(&["depth_per_pass"])
        .with_help("Vertical distance between waterline Z levels."),
    ParamDef::required("sampling", "f64").with_help("XY grid resolution for push-cutter sampling."),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("continuous", "bool"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
    // G-LINKSTAGE (2026-09-09). Absent, or 0.0, is OFF and byte-identical.
    ParamDef::optional("hookup_mm", "f64"),
];

const PENCIL_PARAMS: &[ParamDef] = &[
    ParamDef::required("bitangency_angle", "f64")
        .with_help("Minimum dihedral angle to detect concave edges. 140-170 deg typical."),
    ParamDef::required("min_cut_length", "f64")
        .with_help("Minimum polyline length to include as a pencil pass."),
    ParamDef::required("hookup_distance", "f64")
        .with_help("Max gap between pencil segments to connect into one pass."),
    ParamDef::required("num_offset_passes", "usize"),
    // CMP-08: `stepover` is the alias the named arm writes onto this
    // field. Pencil has no `stepover` field of its own.
    ParamDef::required("offset_stepover", "f64")
        .with_aliases(&["stepover"])
        .with_help("Lateral step between offset cleanup passes around pencil traces."),
    ParamDef::required("sampling", "f64").with_help("XY grid resolution for push-cutter sampling."),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64").with_help(
        "Finishing allowance kept on the surface for a later pass. Applied as \
         a vertical offset: on a wall sloped at angle A, what remains \
         measured normal to the surface is this value x cos(A).",
    ),
    ParamDef::required("min_valley_depth", "f64").with_help(
        "Keep only valleys this much deeper than the reference finish tool \
         reaches. 0 = trace every detected valley.",
    ),
    ParamDef::required("bisector_strength", "f64"),
    ParamDef::required("reference_tool_diameter", "f64").with_help(
        "Nominal reference-tool diameter the rest gate measures against. \
         Applies only when no library tool is chosen.",
    ),
    // G-SCHEMAENUM / FIN-09: the field is `PencilDetector`, not a string.
    ParamDef::required("detector", "enum:dihedral|curvature|rest_depth"),
    ParamDef::required("valley_saliency", "f64").with_help(
        "Smallest concave curvature (1/mm) a valley must reach. Low traces \
         every seam; high keeps only deep sharp valleys.",
    ),
    ParamDef::required("curvature_smoothing", "usize"),
    ParamDef::required("rest_cell_mm", "f64")
        .with_help("XY grid resolution of the rest field the rest-depth detector reads."),
    ParamDef::optional_desc(
        "reference_tool_id",
        "option<usize>",
        "Library tool id whose real geometry defines the pencil rest reference (else nominal diameter)",
    ),
    ParamDef::optional_desc(
        "link_hop_distance_mm",
        "option<f64>",
        "Reach (mm) of the CLEARANCE-HOP link tier only; hookup_distance caps the at-depth \
         tier. Absent = one cap for both tiers (the shipped emission, byte for byte) — absent \
         does NOT mean the hop tier is off. 0 refuses every hop and keeps the at-depth tier, \
         which is the control arm that separates the two tiers.",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const SCALLOP_PARAMS: &[ParamDef] = &[
    ParamDef::required("scallop_height", "f64")
        .with_help("Target cusp height between passes. 0.05-0.2mm for finishing."),
    ParamDef::required("tolerance", "f64").with_help(
        "Geometric tolerance for path approximation. Smaller = more accurate, \
         slower.",
    ),
    // G-SCHEMAENUM: was `x|y`, a raster-axis dial `ScallopConfig` does not
    // have. `ScallopDirection` is outside-in or inside-out.
    ParamDef::required("direction", "enum:outside_in|inside_out"),
    ParamDef::required("continuous", "bool"),
    ParamDef::required("slope_from", "f64").with_help(
        "Minimum surface slope (degrees) to machine. Faces shallower than \
         this are skipped.",
    ),
    ParamDef::required("slope_to", "f64").with_help(
        "Maximum surface slope (degrees) to machine. Steeper faces are \
         skipped.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64").with_help(
        "Finishing allowance kept on the surface for a later pass. Applied as \
         a vertical offset: on a wall sloped at angle A, what remains \
         measured normal to the surface is this value x cos(A).",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
    // A/M7: the ring-to-ring stay-down relink cap. Ships ON at 3.0 mm since
    // wave 12 — see `default_scallop_intra_pass_hookup_mm` for the A/B and
    // for what wave 11's blocking gate was actually measuring.
    ParamDef::required("intra_pass_hookup_mm", "f64"),
    // M8 (2026-09-03) — iso-field ring source; absent = legacy cascade.
    ParamDef::optional("iso_field", "bool"),
];

const UNIFIED_FINISH_PARAMS: &[ParamDef] = &[
    ParamDef::required("steep_threshold_deg", "f64")
        .with_help("Slope entering the mid-steep scallop band (deg). Below this: raster."),
    ParamDef::required("waterline_threshold_deg", "f64").with_help(
        "Slope entering the very-steep waterline band (deg). Above this: \
         waterline.",
    ),
    ParamDef::required("overlap_mm", "f64").with_help("Overlap between steep and shallow regions."),
    ParamDef::required("scallop_height", "f64")
        .with_help("Target cusp height between passes. 0.05-0.2mm for finishing."),
    ParamDef::required("tolerance", "f64").with_help(
        "Geometric tolerance for path approximation. Smaller = more accurate, \
         slower.",
    ),
    ParamDef::required("raster_stepover", "f64")
        .with_help("Distance between raster passes in the shallow band."),
    ParamDef::required("z_step", "f64").with_help("Vertical distance between waterline Z levels."),
    ParamDef::required("sampling", "f64").with_help("XY grid resolution for push-cutter sampling."),
    ParamDef::required_desc(
        "stock_to_leave",
        "f64",
        "Material left on the finished surface (mm), applied as a VERTICAL +Z offset on \
         the cut. Honoured by all three bands (shallow raster, mid-steep scallop, \
         very-steep waterline) since 2026-08-06 — before that only the scallop band \
         applied it. Vertical, not surface-normal: on a wall at angle theta from \
         horizontal what remains measured normal to the surface is stock_to_leave * \
         cos(theta).",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    // v3 S1/S2 claims pipeline: serde-defaulted for project-file back-compat
    // (older files omit them), always serialized.
    ParamDef::required("pencil_claims", "bool"),
    ParamDef::required("min_rest_depth_mm", "f64").with_help(
        "Smallest rest depth a claimed region must reach before the pencil \
         pass cuts it.",
    ),
    // Which reference the crease/rest detector runs against
    // (`unified_finish::ClaimsReference` doc). A/M6 widened it to three
    // values: `auto` DERIVES the answer from whether a machined prior stock
    // is in scope; the other two pin it and keep their exact pre-A/M6
    // meanings. What `auto` resolved to is not a param — read it from the
    // toolpath's `runtime.claims_reference` in `get_toolpath_params`, or
    // from the `config.claims_reference` diagnostic.
    ParamDef::required("claims_reference", "enum:auto|self_probe|machined_stock"),
    // S4 region-level territory clip (`unified_finish::ClaimsConfig::
    // territory_clip` doc) — same serde-defaulted back-compat treatment.
    ParamDef::required("territory_clip", "bool"),
    // C2 shallow-band monotone cell decomposition — same serde-defaulted
    // back-compat treatment again.
    ParamDef::required_desc(
        "monotone_cell_decomposition",
        "bool",
        "Split each SHALLOW region into monotone CELLS on the region's own raster lattice, \
         and rotate that lattice to the region's PCA-minor axis when its elongation clears \
         3.0. Every cell is rastered on that ONE shared lattice, so the relinker sees \
         cell-shaped fragments. Default true since 2026-09-01 (C4 operator surface review \
         passed); set false for the pre-C2 op byte-for-byte. NOT a \
         per-cell strategy: per-cell sweep direction (0.917x), cell TSP (byte-identical to \
         emission order) and contour-per-cell (0.686x) were each measured and refuted. \
         Measured value under a realistic machined-stock link ceiling: 1.155x across the \
         reference relief's top-three shallow regions, 1.215x on the one region that clears \
         the elongation gate — rig figures, not promises. Cell seams change the cusp \
         pattern, which is why a rendered-surface review bound adoption.",
    ),
    // §9/§11 link caps, both serde-defaulted for back-compat:
    // `intra_region_hookup_mm` is the per-region stay-down relink cap,
    // `crease_hookup_mm` the crease node's (see `unified_finish::
    // ClaimsConfig::crease_hookup_mm`).
    ParamDef::required("intra_region_hookup_mm", "f64"),
    ParamDef::required("crease_hookup_mm", "f64"),
    // CMP-09: the field is read at `execute/finish_3d.rs` and reaches
    // `unified_finish`, but no surface could set it, so it was frozen at
    // `ClassificationSampler::PRODUCTION`. `optional` because the config
    // omits the key while it holds that value.
    ParamDef::optional_desc(
        "classification_sampler",
        "enum:drop_cutter_probe|drop_cutter_probe_scratch|triangle_raster|vertical_ray|tile_raster",
        "Which sampler fills the classification height grid (M3 COLUMNS). NOT a quality or \
         speed dial: it exists so an A/B can drive the pre-switch drop-cutter classifier and \
         the production tile-raster one through one pipeline, and so a project that hits a \
         regression has an escape hatch that needs no rebuild. Absent = tile_raster, the \
         production sampler.",
    ),
    // F2 island-filter overrides on `FinishPlannerParams`. All three are
    // `null` by default and are OMITTED from a saved project while null, so
    // an existing file round-trips byte-identically. `null` = derive from the
    // tool (`FinishPlannerParams::for_tool`); a number overrides that ONE
    // dial and leaves the rest derived.
    ParamDef::optional_desc(
        "min_region_area_mm2",
        "option<f64>",
        "Island absorption floor (mm^2) for the finish planner's min-area step: a connected \
         band island smaller than this is absorbed into its surrounding band. null = derive \
         from the tool as (2 * cusp_radius)^2 * 4 (roughly four tool-diameters^2). CUSP \
         radius, not envelope: on a 1 mm-tip / 6 mm-shank taper the derived value is 4 mm^2, \
         not 144 mm^2.",
    ),
    ParamDef::optional_desc(
        "close_radius_mm",
        "option<f64>",
        "Island merge radius (mm) — the morphological close radius applied to each band mask \
         before regions are extracted. Larger merges neighbouring islands into one region. \
         null = derive from the tool as cusp_radius * 0.5 (cusp radius, not envelope).",
    ),
    ParamDef::optional_desc(
        "hysteresis_deg",
        "option<f64>",
        "Band hysteresis width (deg): a cell leaves a band only once its slope falls below \
         (enter - hysteresis_deg). null = the planner's fixed 10.0. Load-bearing — at 0 the \
         raw slope masks storm to O(100) speckled islands.",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const STEEP_SHALLOW_PARAMS: &[ParamDef] = &[
    ParamDef::required("threshold_angle", "f64")
        .with_help("Angle dividing steep (waterline) from shallow (raster) regions."),
    ParamDef::required("overlap_distance", "f64")
        .with_help("Overlap between steep and shallow regions."),
    ParamDef::required("wall_clearance", "f64").with_help("Extra clearance from vertical walls."),
    ParamDef::required("steep_first", "bool"),
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("z_step", "f64").with_help("Vertical distance between waterline Z levels."),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("sampling", "f64").with_help("XY grid resolution for push-cutter sampling."),
    ParamDef::required("stock_to_leave", "f64").with_help(
        "Finishing allowance kept on the surface for a later pass. Applied as \
         a vertical offset: on a wall sloped at angle A, what remains \
         measured normal to the surface is this value x cos(A).",
    ),
    ParamDef::required("tolerance", "f64").with_help(
        "Geometric tolerance for path approximation. Smaller = more accurate, \
         slower.",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const RAMP_FINISH_PARAMS: &[ParamDef] = &[
    // CMP-08: the `depth_per_pass` alias, as on Waterline's `z_step`.
    ParamDef::required("max_stepdown", "f64")
        .with_aliases(&["depth_per_pass"])
        .with_help("Maximum Z step between ramp passes."),
    ParamDef::required("slope_from", "f64").with_help(
        "Minimum surface slope (degrees) to machine. Faces shallower than \
         this are skipped.",
    ),
    ParamDef::required("slope_to", "f64").with_help(
        "Maximum surface slope (degrees) to machine. Steeper faces are \
         skipped.",
    ),
    ParamDef::required("direction", "enum:climb|conventional"),
    ParamDef::required("order_bottom_up", "bool"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("sampling", "f64").with_help("XY grid resolution for push-cutter sampling."),
    ParamDef::required("stock_to_leave", "f64").with_help(
        "Finishing allowance kept on the surface for a later pass. Applied as \
         a vertical offset: on a wall sloped at angle A, what remains \
         measured normal to the surface is this value x cos(A).",
    ),
    ParamDef::required("tolerance", "f64").with_help(
        "Geometric tolerance for path approximation. Smaller = more accurate, \
         slower.",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const SPIRAL_FINISH_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    // G-SCHEMAENUM: was `outward|inward`; `SpiralDirection` spells the same
    // two directions `inside_out` and `outside_in`.
    ParamDef::required("direction", "enum:inside_out|outside_in"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64").with_help(
        "Finishing allowance kept on the surface for a later pass. Applied as \
         a vertical offset: on a wall sloped at angle A, what remains \
         measured normal to the surface is this value x cos(A).",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const RADIAL_FINISH_PARAMS: &[ParamDef] = &[
    ParamDef::required_ranged(
        "angular_step",
        "f64",
        ParamRange::greater_than(0.0),
        "Angle (degrees) between two spokes. Must be strictly positive \
         and finite: `radial_finish` computes the spoke count as \
         `360.0 / angular_step`, so a zero gives `inf` and casts to \
         `usize::MAX` spokes, and a negative value casts to zero spokes \
         and emits nothing (N10).",
    ),
    ParamDef::required_ranged(
        "point_spacing",
        "f64",
        ParamRange::greater_than(0.0),
        "Distance (mm) between two sample points along one spoke. Must \
         be strictly positive and finite: `radial_finish` computes the \
         point count as `max_radius / point_spacing`, so a zero \
         overflows the point allocation and a negative value leaves one \
         point per spoke and emits nothing (N10).",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64").with_help(
        "Finishing allowance kept on the surface for a later pass. Applied as \
         a vertical offset: on a wall sloped at angle A, what remains \
         measured normal to the surface is this value x cos(A).",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const HORIZONTAL_FINISH_PARAMS: &[ParamDef] = &[
    ParamDef::required("angle_threshold", "f64")
        .with_help("Max slope angle (degrees) to consider a surface flat/horizontal."),
    ParamDef::required("stepover", "f64").with_help(
        "Distance between passes. 40-60% of diameter for roughing, 10-20% for \
         finishing.",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64").with_help(
        "Finishing allowance kept on the surface for a later pass. Applied as \
         a vertical offset: on a wall sloped at angle A, what remains \
         measured normal to the surface is this value x cos(A).",
    ),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const PROJECT_CURVE_PARAMS: &[ParamDef] = &[
    ParamDef::required("depth", "f64").with_help("Total cut depth from stock surface."),
    ParamDef::required("point_spacing", "f64")
        .with_help("Distance between sample points along curves. Smaller = smoother."),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::optional("surface_model_id", "option<usize>"),
    ParamDef::required("direction", "enum:from_above|from_below"),
    ParamDef::required("side", "enum:center|inside|outside"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
    ParamDef::required_ranged(
        "chain_distance_mm",
        "f64",
        ParamRange::at_least(0.0),
        "Cap (mm) on the XY gap between two projected chains that may be \
         joined by one clearance-height link instead of a full \
         retract/rapid/replunge round trip. `0.0` (the default) disables \
         chaining and emits exactly what this operation always did. The \
         value is a CAP, not a target: every candidate inside it is still \
         drop-cutter sampled for gouge, refused if it would leave the \
         operation's boundary, lifted clear of standing material, and \
         refused outright when that clearance reaches safe Z.",
    ),
];

const ALIGNMENT_PIN_DRILL_PARAMS: &[ParamDef] = &[
    ParamDef::required("holes", "array<[f64;2]>"),
    ParamDef::required("spoilboard_penetration", "f64")
        .with_help("How far the drill penetrates into the spoilboard below the stock."),
    ParamDef::required("cycle", "enum:simple|dwell|peck|chip_break"),
    ParamDef::required_ranged(
        "peck_depth",
        "f64",
        ParamRange::greater_than(0.0),
        "Depth of each peck (mm), rooted at the R-plane like Fanuc G83. \
         Must be strictly positive and finite: `drill::fed_descents` \
         degrades a zero, negative or non-finite peck to a single \
         full-depth descent, so any such value silently turns a Peck \
         cycle into a single-shot one (DR-LIVE).",
    ),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("retract_z", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

pub(super) static REG_FACE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Face,
    spec: OperationSpec {
        label: "Face",
        description: "Level the stock top surface",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Stock,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Pocket,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: FACE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_face,
};

pub(super) static REG_POCKET: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Pocket,
    spec: OperationSpec {
        label: "Pocket",
        description: "Clear material inside a closed region",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Pocket,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: POCKET_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_pocket,
};

pub(super) static REG_PROFILE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Profile,
    spec: OperationSpec {
        label: "Profile",
        description: "Cut along the outside or inside of a boundary",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Contour,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Contour,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: PROFILE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_profile,
};

pub(super) static REG_ADAPTIVE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Adaptive,
    spec: OperationSpec {
        label: "Adaptive",
        description: "Constant-engagement rough clearing",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Adaptive,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: ADAPTIVE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    // Roadmap B.5 — 2D adaptive pocketing has natural circular
    // boundaries, so Ramp upgrades to Helix.
    dressup_policy: DressupPolicy::PREFER_HELIX,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_adaptive,
};

pub(super) static REG_VCARVE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::VCarve,
    spec: OperationSpec {
        label: "VCarve",
        description: "V-bit engraving with variable depth",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: VCARVE_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_kinds: &[CutterKind::VBit],
        supports_v_bit: true,
    },
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_vcarve,
};

pub(super) static REG_REST: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Rest,
    spec: OperationSpec {
        label: "Rest Machining",
        description: "Clean up areas a larger tool couldn\u{2019}t reach",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Pocket,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: REST_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_rest,
};

pub(super) static REG_INLAY: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Inlay,
    spec: OperationSpec {
        label: "Inlay",
        description: "V-bit pocket and plug for inlay work",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: INLAY_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_kinds: &[CutterKind::VBit],
        supports_v_bit: true,
    },
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_inlay,
};

pub(super) static REG_ZIGZAG: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Zigzag,
    spec: OperationSpec {
        label: "Zigzag",
        description: "Back-and-forth raster clearing at an angle",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Pocket,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: ZIGZAG_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_zigzag,
};

pub(super) static REG_TRACE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Trace,
    spec: OperationSpec {
        label: "Trace",
        description: "Follow a path exactly for engraving or scoring",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: TRACE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    // Roadmap B.5 — single-pass engraving: no entry style applies.
    dressup_policy: DressupPolicy::FORCE_NO_ENTRY,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_trace,
};

pub(super) static REG_DRILL: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Drill,
    spec: OperationSpec {
        label: "Drill",
        description: "Drill holes at the drawing's DXF points and circle/arc centres, and \
                      circles in SVG drawings (pick targets in the viewport or by layer; \
                      refuses when there are none)",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Drill,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(DRILL_FORMULA_SOURCE),
    },
    param_defs: DRILL_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    // Roadmap B.5 — stock-based peck cycle: no entry style applies.
    dressup_policy: DressupPolicy::FORCE_NO_ENTRY,
    policy: OpPolicy::DRILLING,
    generate: crate::compute::execute::generate_drill,
};

pub(super) static REG_CHAMFER: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Chamfer,
    spec: OperationSpec {
        label: "Chamfer",
        description: "Bevel edges with a V-bit",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: CHAMFER_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_kinds: &[CutterKind::VBit],
        supports_v_bit: true,
    },
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_chamfer,
};

pub(super) static REG_DROP_CUTTER: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::DropCutter,
    spec: OperationSpec {
        label: "3D Finish",
        description: "Parallel raster passes following the surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Parallel,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Parallel,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: DROP_CUTTER_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    // Each raster segment that starts with a Ramp entry would cut a
    // diagonal line from safe_z down to the mesh surface — hundreds per
    // 3D finish, covering the stock in angled trenches. Lead-in/out add
    // arc transitions that produce more diagonals on a zigzag raster.
    dressup_policy: DressupPolicy::strip_all(
        "Incompatible with 3D Finish: each raster segment's ramp entry would carve a diagonal trench across the stock.",
    ),
    policy: OpPolicy::LATERAL_RASTER,
    generate: crate::compute::execute::generate_drop_cutter,
};

pub(super) static REG_ADAPTIVE3D: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Adaptive3d,
    spec: OperationSpec {
        label: "3D Rough",
        description: "Load-limiting rough mill on a 3D surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Adaptive,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: ADAPTIVE3D_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    // F-031 (2026-05-26): the planner already emits its entry sequence
    // per `Adaptive3dParams::entry_style` and stamps it into its
    // internal material_stock. A dressup-level helix/ramp replacement
    // breaks planner↔simulator stamp parity (observed: ~44 mm axial
    // engagement on a 3 mm-commanded DPP, deflection gate Exceeds).
    // Users who want a Helix entry set it at the planner level.
    dressup_policy: DressupPolicy::FORCE_NO_ENTRY,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_adaptive3d,
};

pub(super) static REG_WATERLINE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Waterline,
    spec: OperationSpec {
        label: "Waterline",
        description: "Horizontal contours at constant Z levels",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Contour,
        ui_process_role: UiProcessRole::SemiFinish,
        feeds_family: FeedsOperationFamily::Contour,
        feeds_pass_role: PassRole::SemiFinish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: WATERLINE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::FEATURE_SELECTIVE,
    generate: crate::compute::execute::generate_waterline,
};

pub(super) static REG_PENCIL: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Pencil,
    spec: OperationSpec {
        label: "Pencil Finish",
        description: "Trace concave edges and creases on the surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: PENCIL_PARAMS,
    // Feeds matrix R1 (2026-09-23): the static check calls a flat end mill
    // on Pencil Critical ("require a ball nose tool for correct surface
    // contact"); the row now says so, and Suggest refuses on it.
    tool_constraints: ToolConstraintsDef {
        required_kinds: &[CutterKind::Ball, CutterKind::TaperedBall],
        supports_v_bit: false,
    },
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::FEATURE_SELECTIVE,
    generate: crate::compute::execute::generate_pencil,
};

pub(super) static REG_SCALLOP: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Scallop,
    spec: OperationSpec {
        label: "Scallop Finish",
        description: "Variable stepover for constant scallop height",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Scallop,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Scallop,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: SCALLOP_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_kinds: &[CutterKind::Ball, CutterKind::TaperedBall],
        supports_v_bit: false,
    },
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_scallop,
};

/// P2.c orchestrator (`planning/unified_finish_planner_design.md`): bands
/// the surface by true-surface slope and runs waterline/scallop/raster per
/// band. No standard depth-stepping applies — `cutting_levels()`'s wildcard
/// arm correctly falls through for this op (each band's Z range is derived
/// internally per-band, not from a single top/bottom depth-per-pass ladder).
pub(super) static REG_UNIFIED_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::UnifiedFinish,
    spec: OperationSpec {
        label: "Unified Finish",
        description: "Bands the surface by true-surface slope and runs waterline/scallop/raster per band",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Scallop,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Scallop,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: UNIFIED_FINISH_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_kinds: &[CutterKind::Ball, CutterKind::TaperedBall],
        supports_v_bit: false,
    },
    // P2.f Task 2 (2026-07-09): mirrors DropCutter — the op emits raster
    // rows, scallop rings, and waterline contours directly on the mesh
    // surface, plus its OWN router-costed links, so a role-default Ramp
    // entry carves ~20 mm diagonal trenches across the terrain at every
    // plunge (live wanaka: entry_s 6× the headless chain + rapid
    // descents below terrain knobs from `emit_ramp`'s target-relative
    // rapid floor). Lead-in/out and dressup-level link moves are wrong
    // for the same reason.
    dressup_policy: DressupPolicy::strip_all(
        "Incompatible with Unified Finish: ramp/lead/link dressups would carve diagonal trenches across the mesh surface; the op emits its own surface-safe entries and links.",
    ),
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_unified_finish,
};

pub(super) static REG_STEEP_SHALLOW: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::SteepShallow,
    spec: OperationSpec {
        label: "Steep/Shallow",
        description: "Waterline on steep areas, raster on shallow",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Contour,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Contour,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: STEEP_SHALLOW_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::LATERAL_RASTER,
    generate: crate::compute::execute::generate_steep_shallow,
};

pub(super) static REG_RAMP_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::RampFinish,
    spec: OperationSpec {
        label: "Ramp Finish",
        description: "Continuous Z descent along contours, no retract",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Parallel,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Parallel,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: RAMP_FINISH_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_ramp_finish,
};

pub(super) static REG_SPIRAL_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::SpiralFinish,
    spec: OperationSpec {
        label: "Spiral Finish",
        description: "Archimedean spiral passes over the surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Scallop,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Scallop,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: SPIRAL_FINISH_PARAMS,
    // Feeds matrix R1 (2026-09-23, EVIDENCE 2-1): the row declares the
    // Scallop feeds family, so it carries Scallop's tool rule. Before, the
    // scallop refusal fired from the family alone while the row accepted
    // any tool.
    tool_constraints: ToolConstraintsDef {
        required_kinds: &[CutterKind::Ball, CutterKind::TaperedBall],
        supports_v_bit: false,
    },
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::LATERAL_RASTER,
    generate: crate::compute::execute::generate_spiral_finish,
};

pub(super) static REG_RADIAL_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::RadialFinish,
    spec: OperationSpec {
        label: "Radial Finish",
        description: "Spoke-pattern passes radiating from center",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Parallel,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Parallel,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: RADIAL_FINISH_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_radial_finish,
};

pub(super) static REG_HORIZONTAL_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::HorizontalFinish,
    spec: OperationSpec {
        label: "Horizontal Finish",
        description: "Finish only flat areas of the surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Parallel,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Parallel,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: HORIZONTAL_FINISH_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    dressup_policy: DressupPolicy::ANY_DRESSUP,
    policy: OpPolicy {
        // The only op that is both: a lateral raster pitch AND a
        // feature-selective empty. It cuts near-flat areas only, so a
        // model with no flats legitimately yields nothing.
        raster_stepover_is_lateral: true,
        kinematics: Kinematics::Milling,
        empty_is_feature_selective: true,
    },
    generate: crate::compute::execute::generate_horizontal_finish,
};

pub(super) static REG_PROJECT_CURVE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::ProjectCurve,
    spec: OperationSpec {
        label: "Project Curve",
        description: "Project 2D curves onto a 3D mesh surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Both,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
        feeds_formula_source: Some(MILLING_FORMULA_SOURCE),
    },
    param_defs: PROJECT_CURVE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    // Entry ramps, lead-in/out, and link moves produce phantom
    // lateral cuts on multi-ring DXFs.
    dressup_policy: DressupPolicy::strip_all(
        "Incompatible with Project Curve: each ring would get a phantom diagonal cut.",
    ),
    policy: OpPolicy::MILLING,
    generate: crate::compute::execute::generate_project_curve,
};

pub(super) static REG_ALIGNMENT_PIN_DRILL: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::AlignmentPinDrill,
    spec: OperationSpec {
        label: "Pin Drill",
        description: "Drill alignment pin holes through stock",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Stock,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Drill,
        feeds_pass_role: PassRole::Roughing,
        feeds_formula_source: Some(DRILL_FORMULA_SOURCE),
    },
    param_defs: ALIGNMENT_PIN_DRILL_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
    // FORCE_NO_ENTRY, matching REG_DRILL. A ramp or helix entry on a
    // registration-pin hole cuts an oval slot, which destroys the flip
    // registration the op exists to provide (G-WANAKA-DRILL-RAMP). The
    // generator hard-strips it either way, but leaving the policy at
    // ANY_DRESSUP meant `DressupConfig::for_op` SHIPPED a Ramp here, so
    // the UI offered a control that silently did nothing and the strip
    // warned on the product's own default.
    dressup_policy: DressupPolicy::FORCE_NO_ENTRY,
    policy: OpPolicy::DRILLING,
    generate: crate::compute::execute::generate_alignment_pin_drill,
};
