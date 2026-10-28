# rs_cam_mcp instructions

This crate owns shared MCP parameter/request-response types. It does **not**
run the server; the live server, GUI dispatch and session ownership are in
`crates/rs_cam_viz`.

- Keep wire schemas explicit and typed. A value with a fixed set of
  legal tokens takes an enum, not a `String` with the tokens in prose:
  a client discovers the legal values from the schema. Mark such an enum
  `#[schemars(inline)]` — the published tool schema carries no `$defs`,
  so a `$ref` does not resolve for a client.
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
- Verify with `cargo test -p rs_cam_mcp -q`; when a wire type changes, also run
  the relevant `rs_cam_viz` MCP integration tests.
