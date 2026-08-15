#!/bin/bash
# S-3 / A2D-165 batch driver. Runs the campaign binary directly (no cargo
# slot held) one batch per invocation, each under its own wall-clock and
# address-space bound, with every measured row flushed to R2_ROW_LOG as it
# lands so a killed batch still leaves its evidence.
SP=/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/0934e428-3a02-4818-b682-76c62c03a29a/scratchpad/a2d
BIN=/home/ricky/personal_repos/rs_cam/target/debug/deps/adversarial_2d_campaign_r2-29c7ce30c73561f7
cd /home/ricky/personal_repos/rs_cam || exit 1
ulimit -v 16777216   # 16 GiB address space: F-10 reached 22.9 GB RSS

run_batch () {
  local tag="$1" fixtures="$2" ops="$3" secs="$4"
  echo "=== BATCH $tag  fixtures=[$fixtures] ops=[${ops:-ALL}] timeout=${secs}s  $(date -Is)"
  R2_ONLY_FIXTURES="$fixtures" R2_ONLY_OPS="$ops" \
  R2_ROW_LOG="$SP/rows.md" R2_ARTIFACT_DIR="$SP/art" \
    timeout "$secs" "$BIN" --ignored --nocapture --exact adversarial_2d_full_campaign \
    > "$SP/logs/$tag.log" 2>&1
  echo "=== BATCH $tag exit=$? $(date -Is)"
}

run_batch b1 "rosette-24"                                             "inlay,drill,rest" 900
run_batch b2 "islands-4x4,holed-9,walls-1e-2,walls-1e-6"              "" 2400
run_batch b3 "short-edges,near-collinear-1nm,tiny-islands"            "" 2400
run_batch b4 "slot-under,slot-exact,high-curvature"                   "" 2400
run_batch b5 "invalid-bowtie,invalid-zero-area,invalid-two-vertex,invalid-nan" "" 2400
run_batch b6 "invalid-cw,invalid-open,invalid-far"                    "" 2400
echo "=== ALL BATCHES DONE $(date -Is)"
