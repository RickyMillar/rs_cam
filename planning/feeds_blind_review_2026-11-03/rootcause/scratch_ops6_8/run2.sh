#!/bin/bash
# Runs rough-score --no-sim on the dressup variants.
cd /home/ricky/personal_repos/rs_cam
D=planning/feeds_blind_review_2026-11-03/rootcause/scratch_ops6_8
run() { name=$1; file=$2; shift 2
  systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 timeout 600 \
    target/release/rs_cam_cli rough-score $D/$file --no-sim "$@" > $D/$name.json 2> $D/$name.err
  echo "$name exit=$?"; head -1 $D/$name.json | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(' moves',d['diagnostic']['move_count'], d['counts'])" 2>/dev/null || tail -n 1 $D/$name.err
}
run op8_none v_op8_none.toml --toolpath 14
run op8_ramp10 v_op8_ramp10.toml --toolpath 14
run op8_clear0 v_op8_clear0.toml --toolpath 14
run op8_clear2 v_op8_clear2.toml --toolpath 14
run op7_ramp v_op7_ramp.toml --toolpath 13
run op6_ramp v_op6_ramp.toml --toolpath 12
