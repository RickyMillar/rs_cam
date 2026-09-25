#!/bin/bash
# Usage (from the repository root, after `cargo build --release -p rs_cam_cli`):
#   F=rivmap100_live_0925.toml planning/fixtures/rivmap100/arm.sh <label> [rough-score args...]
# Prints one line: moves, total / cut / entry / rapid time, removed volume,
# peak DOC and whole-cycle engagement of toolpath 1 (the 3D Rough).
D="$(cd "$(dirname "$0")" && pwd)"
n="$1"; shift
target/release/rs_cam_cli rough-score "$D/${F:-rivmap100_ladder_demo.toml}" --toolpath 1 --resolution 0.5 "$@" 2>/dev/null | python3 "$D/fmt.py" "$n" || echo "$n FAILED"
