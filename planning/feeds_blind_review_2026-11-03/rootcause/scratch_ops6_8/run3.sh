#!/bin/bash
cd /home/ricky/personal_repos/rs_cam
D=planning/feeds_blind_review_2026-11-03/rootcause/scratch_ops6_8
run() { name=$1; file=$2; shift 2
  systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 timeout 600 \
    target/release/rs_cam_cli rough-score $D/$file --no-sim "$@" > $D/$name.json 2> $D/$name.err
  echo "$name exit=$?"; head -1 $D/$name.json | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(' moves',d['diagnostic']['move_count'], d['counts'])" 2>/dev/null || tail -n 1 $D/$name.err
}
run op8_ramp1 v_op8_ramp1.toml --toolpath 14
run op8_nomerge v_op8_nomerge.toml --toolpath 14
