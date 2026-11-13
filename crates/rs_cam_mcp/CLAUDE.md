# rs_cam_mcp instructions

This crate owns shared MCP parameter/request-response types. It does **not**
run the server; the live server, GUI dispatch and session ownership are in
`crates/rs_cam_viz`.

- Keep wire schemas explicit and typed. A value with a fixed set of
  legal tokens takes an enum, not a `String` with the tokens in prose:
  a client discovers the legal values from the schema. Mark such an enum
  `#[schemars(inline)]` — the published tool schema carries no `$defs`,
  so a `$ref` does not resolve for a client.
- The non-centre-cutting tool geometry is one measured object; do not split it into partial setters. K3 (feeds contract P6, 2026-11-04): `AddToolParam.centre_cutting` is `CentreCuttingParam` (`"yes"` or `{"no": geometry}`), and `non_centre_cutting` is gone (break, no alias); an omitted fact is `Unknown`; `set_tool_param` takes `centre_cutting` (`"yes"`, the geometry object, or `null`). The snapshot was re-blessed.
- A wire enum that mirrors a core enum converts with an exhaustive
  `match` in BOTH directions (`server.rs`, F166), so a variant that core
  gains does not compile until the wire carries it (S11: `FloorParam::Unset`
  is the floor that is not set). A wire patch of a
  core struct converts by a full destructure with no `..` on both sides
  (`SetStockConfigParam` → `StockConfigPatch`, F118). Free text stays a
  `String`: names, paths, layers, the `material` name, overlay ids.
- Breaking a wire shape is allowed (operator ruling 2026-09-16, no
  legacy support). State the break in the commit; do not add an alias.
  A `build_info().features` token records a shipped CAPABILITY, not a key
  name: add one when a reply gains a key, and keep the old one when a key
  is renamed.
- Every type that derives `Deserialize` in `server.rs` carries
  `#[serde(deny_unknown_fields)]` (F26). An unknown field gives an MCP
  error that names the field. Do not use `#[serde(flatten)]` with it.
  Sentry: `an_unknown_mcp_field_is_refused_f26` (it also scans the file).
- A cap on a response array is named here, in `response.rs`, never
  written as a number in a handler.
- A schema change normally requires coordinated updates to this crate, the
  viz server/handler and MCP surface tests.
- Do not put controller, GUI state or `ProjectSession` mutation logic here.
  Those belong in viz/core respectively.
- R2/Q7 (2026-10-24): `AddToolpathParam` `tool_index` and `model_id` are optional (the first
  fitting tool; the core model rule), `AddToolpathViaGuiParam` has `model_id`, both replies carry
  `no_tool`, and the wire snapshot was re-blessed.
- A C5 (2026-10-29): `SetUiViewParam.camera` is one `CameraParam` (`preset`, `projection`, `fit`,
  `pose`), inline structs; viz `state::camera` holds the `From` mirrors. The snapshot was re-blessed.
- E3 (2026-10-29): `SelectionEditParam` has `add_picks`/`remove_picks`; the core -> wire `From`
  of `SelectionEdit`, `Selection`, `Rule`, `Filter` (and their parts) is exhaustive (F166/F118).
- E5 (2026-10-29): `ListRuleOffersParam` (`list_rule_offers`); the reply `edit` is the
  `SelectionEditParam` JSON. The snapshot was re-blessed.
- FOLLOW_UPS §1 (2026-10-29): `SetUiViewParam.display_mode` (`DisplayModeParam`, inline; `From` both ways with core
  `settings::DisplayMode`). The snapshot was re-blessed.
- W1 (2026-10-30): a reply `Diagnostic` carries `face` and `anchor` (core `diagnostics::presentation`);
  `list_toolpaths` rows carry `run_state` and `next_action` (core `session::toolpath_run_state`).
  Feature tokens `diagnostic_face_anchor`, `toolpath_run_state`. Reply keys only: the snapshot did not move.
- W2b (2026-10-30): `PropertiesTabParam` is `geometry`, `passes`, `feeds`, `heights`, `linking`;
  `cut` and `dressup` are deleted (no-legacy ruling). `list_toolpaths` `run_state` gains `waiting`
  (`{"state": "waiting", "on": [ids]}`). The snapshot was re-blessed (the enum only).
- Feeds contract P5 (2026-11-03, K2b): `ApplyScopeParam` is deleted (break, no alias).
  `ApplyFeedsParam { index, pick: Option<PickParam> }`; `PickParam` (inline: `proposal`,
  `point { ap_mm, ae_mm }`, `peck { mm }`) converts both ways with core `PickRequest` (F166).
  No pick writes the speeds for the cut the op keeps. The reply carries `unsourced_rows` and
  `outside_envelope`. New tool `get_safe_envelope { index }` (the grid cap is
  `response::MAX_SAFE_ENVELOPE_GRID_POINTS`); `get_suggest_rationale` `basis.vendor_band`
  (P16 item 8; it replaced `basis.tied_rows`). Feature tokens `safe_envelope`, `apply_feeds_pick`. The snapshot was re-blessed.
- Feeds contract P5b (2026-11-04, K8): new tool `get_toolpath_lineage { index }` (the
  core `Lineage::to_json`: cut, pick, speeds with stamps, tool facts, entry, findings
  with consequence, rules). Feature token `toolpath_lineage`. `set_ui_view`
  `properties_tab: "feeds"` opens the Passes tab (the Feeds tab merged into it; the
  wire enum is unchanged). The snapshot was re-blessed.
- Feeds contract P21b (2026-11-10): `SpindleStrategyParam` mirrors core `SpindleStrategyKind`
  (`match_chart`, `constant_chip`; `max_speed` is gone, break, no alias), and
  `SetSpindleStrategyParam` has `rpm_ceiling` (`[job.post] spindle_rpm_ceiling`; 0 refuses, a
  value above the spindle maximum warns in the reply). Feature token `spindle_rpm_ceiling`. The
  snapshot was re-blessed.
- Feeds contract P29-10 (2026-11-13, 10a): `get_safe_envelope` `envelope.cut_checks` is now
  `envelope.limit_rows` (one `limits::CheckRow` per record: subject, check, caveats, exemption, tone,
  face, detail; the planned rows, then the stated rows); a `Check` has no `bound_detail` (break, no
  alias). Feature token `limit_rows`. Reply keys only: the snapshot did not move.
- P29-10 (10b): `envelope.notes` is `[{kind, face, text}]` (core `EnvelopeNote`, a closed list:
  `no_depth`, `move_budget_not_met`, `computed_width`, `force_budget_floor`, `move_budget_conflict`);
  the may-occur sentence left the notes (break). Feature token `envelope_note_kinds`.
- P29-10 (10c): `summary.exceeds_breakdown[].group` and a diagnostic's `group` (a group
  load finding; absent for every other finding). Feature token `load_finding_group`. Additions only.
- P29-10 (10d): the three P29-2 `checks` keys are deleted (break, no alias):
  `get_tool_load_report` `limit_rows` (`[{toolpath_id, toolpath_name, rows}]`, every gate and cut
  group), `get_suggest_rationale` `limit_rows` (the RPM rows), and `get_safe_envelope` reads only
  `envelope.limit_rows`. One door, `ProjectSession::limit_rows`. Feature token `limit_rows_every_reply`.
- P29-10 (10x, lead rulings 2, 3, 6): a row carries `compared` (`load`, `depth`, `width`); a depth
  row's face names the depth; `stored_feed` holds only where the rounding decides the state; every
  toolpath has a `get_tool_load_report` `limit_rows` entry (no-simulation rows when it has no verdict).
- P29-10 (10e): `SetUiViewParam` gains `sim_inspector_tab` (`SimInspectorTabParam`) and `sim_cut_groups`
  (`DrawerParam`: `open`, `closed`), string enums so a client with an old schema still sends them; the reply
  echoes `sim_inspector`. The lineage `measured.binding` and `measured.judged` are `measured.row` (break);
  `load_report.per_toolpath[].groups` left the wire (break). Tokens `sim_inspector_view`, `lineage_measured_row`.
  The snapshot was re-blessed (additions only).
- P29-10 (10f): `SetUiViewParam` gains `card_limits` (`DrawerParam`): the "Limits" drawer of the Suggest
  card; the reply echoes `card_limits`. Token `card_limits_view`. The snapshot was re-blessed (additions only).
- P29-10 (10g): `get_safe_envelope` gains `plane_marks` ({proposal, stated}), `force_budget_columns` and
  `outside_cut`; a planned row's `subject.own_depth_mm` is `subject.at` ({ae_mm, ap_mm, own_depth}, break).
  Token `envelope_plane_marks`. Reply keys only: the snapshot did not move.
- Verify with `cargo test -p rs_cam_mcp -q`; when a wire type changes, also run
  the relevant `rs_cam_viz` MCP integration tests.
