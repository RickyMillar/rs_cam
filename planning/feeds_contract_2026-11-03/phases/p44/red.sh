#!/bin/bash
# P44b red proofs. The saved copies and the logs go beside this script; run a copy in a scratch folder.
# usage: red.sh <H1|H2|H3|RULE> <test name filter...> ; reverts one hunk, runs the tests, restores, checks the tree
S=$(cd "$(dirname "$0")" && pwd); R=/home/ricky/personal_repos/rs_cam; k=$1; shift
cd $R; before=$(git diff | sha256sum)
cp crates/rs_cam_core/src/adaptive3d/clearing.rs $S/keep_clearing.rs; cp crates/rs_cam_core/src/adaptive3d/left_stock.rs $S/keep_left_stock.rs
python3 $S/revert.py $k || { echo "revert failed"; exit 2; }
git diff --stat -- crates/rs_cam_core/src/adaptive3d/clearing.rs crates/rs_cam_core/src/adaptive3d/left_stock.rs | tail -1
timeout 3000 scripts/cargo_lane.sh test -p rs_cam_core -q --lib -j 2 -- "$@" > $S/red_$k.log 2>&1; echo "rc $?" >> $S/red_$k.log
cp $S/keep_clearing.rs crates/rs_cam_core/src/adaptive3d/clearing.rs; cp $S/keep_left_stock.rs crates/rs_cam_core/src/adaptive3d/left_stock.rs
after=$(git diff | sha256sum); [ "$before" = "$after" ] && echo "tree restored" || echo "TREE DIFFERS"
grep -a "^test \|test result\|^rc \|panicked\|error" $S/red_$k.log | cut -c1-330 | head -30
