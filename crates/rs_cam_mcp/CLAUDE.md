# rs_cam_mcp instructions

This crate owns shared MCP parameter/request-response types. It does **not**
run the server; the live server, GUI dispatch and session ownership are in
`crates/rs_cam_viz`.

- Keep wire schemas explicit and typed. A value with a fixed set of
  legal tokens takes an enum, not a `String` with the tokens in prose:
  a client discovers the legal values from the schema. Mark such an enum
  `#[schemars(inline)]` — the published tool schema carries no `$defs`,
  so a `$ref` does not resolve for a client.
- Breaking a wire shape is allowed (operator ruling 2026-09-16, no
  legacy support). State the break in the commit; do not add an alias.
  A `build_info().features` token records a shipped CAPABILITY, not a key
  name: add one when a reply gains a key, and keep the old one when a key
  is renamed.
- A cap on a response array is named here, in `response.rs`, never
  written as a number in a handler.
- A schema change normally requires coordinated updates to this crate, the
  viz server/handler and MCP surface tests.
- Do not put controller, GUI state or `ProjectSession` mutation logic here.
  Those belong in viz/core respectively.
- Verify with `cargo test -p rs_cam_mcp -q`; when a wire type changes, also run
  the relevant `rs_cam_viz` MCP integration tests.
