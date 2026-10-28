# Evidence: live screenshots

All from the release binary built from master 994d613ad on 2026-10-29,
through the MCP proxy (`screenshot_gui`, 1400x900).

| File | What it shows |
|---|---|
| `01_default_view_after_import.png` | `stepped_block.step` after import plus a Pocket: 4x MSAA on the edges, the ink face edges (D4), smooth faces (D2), the selection states. |
| `02_preset_front_via_mcp.png` | `set_ui_view {camera: {preset: "front"}}`; the echo gave yaw -90, pitch 0, `view_preset: "front"`. Another session was working in the same GUI at that moment (a second setup and a Profile 2 appeared), and the selected toolpath is in that setup, so the picture also carries that setup's display frame. Take the preset pairs again on a quiet GUI. |
