# rs_cam_mcp instructions

This crate owns shared MCP parameter/request-response types. It does **not**
run the server; the live server, GUI dispatch and session ownership are in
`crates/rs_cam_viz`.

- Keep wire schemas explicit, typed and backwards-conscious.
- A schema change normally requires coordinated updates to this crate, the
  viz server/handler and MCP surface tests.
- Do not put controller, GUI state or `ProjectSession` mutation logic here.
  Those belong in viz/core respectively.
- Verify with `cargo test -p rs_cam_mcp -q`; when a wire type changes, also run
  the relevant `rs_cam_viz` MCP integration tests.
