#!/bin/bash
# P45 red proofs. usage: red.sh <scratch dir> <PASS|READ|SCAN|EDGE> <test name filter...>
# Reverts one hunk, runs the lib tests, restores the files, checks that the tree is as before.
H=$(cd "$(dirname "$0")" && pwd); S=$1; R=/home/ricky/personal_repos/rs_cam; k=$2; shift 2
cd $R; before=$(git diff | sha256sum); A=crates/rs_cam_core/src/adaptive3d
command cp -f $A/level_contour.rs $S/keep_level_contour.rs; command cp -f $A/path.rs $S/keep_path.rs
python3 $H/revert.py $k || { echo "revert failed"; exit 2; }
git diff --stat -- $A/level_contour.rs $A/path.rs | tail -1
timeout 3000 scripts/cargo_lane.sh test -p rs_cam_core -q --lib -j 2 -- "$@" > $S/red_$k.log 2>&1; echo "rc $?" >> $S/red_$k.log
command cp -f $S/keep_level_contour.rs $A/level_contour.rs; command cp -f $S/keep_path.rs $A/path.rs
after=$(git diff | sha256sum); [ "$before" = "$after" ] && echo "tree restored" || echo "TREE DIFFERS"
echo "== hunk $k reverted"; grep -a "^test \|test result\|^rc \|panicked\|^error\|stands at\|cuts a node\|keeps its\|reads the cut" $S/red_$k.log | cut -c1-330 | head -30
