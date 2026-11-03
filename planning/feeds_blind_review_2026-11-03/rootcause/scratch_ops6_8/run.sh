#!/bin/bash
# Runs rough-score --no-sim experiments for ops 6, 7, 8 on the scratch copy.
cd /home/ricky/personal_repos/rs_cam
D=planning/feeds_blind_review_2026-11-03/rootcause/scratch_ops6_8
run() { name=$1; shift
  systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 timeout 600 \
    target/release/rs_cam_cli rough-score $D/wanaka21_ops6_8.toml --no-sim "$@" > $D/$name.json 2> $D/$name.err
  echo "$name exit=$?"; head -1 $D/$name.json | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(' moves',d['diagnostic']['move_count'], d['counts'])"
}
run op8_base --toolpath 14
run op8_none --toolpath 14 --set 8.entry_style=none
run op8_ramp10 --toolpath 14 --set 8.ramp_angle=10
run op8_clear0 --toolpath 14 --set 8.entry_clearance_mm=0
run op8_dpp5 --toolpath 14 --set 8.depth_per_pass=5
run op6_base --toolpath 12
run op7_base --toolpath 13
run op6_ramp --toolpath 12 --set 6.entry_style=ramp
run op8_suggest --toolpath 14 --set 8.depth_per_pass=0.3333 --set 8.stepover=0.47
