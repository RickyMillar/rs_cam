#!/bin/bash
# P46 red proofs. usage: red.sh <scratch dir> <hunk> <test name filter...>
# Reverts one hunk, runs the lib tests, restores the files, checks that the tree is as before.
H=$(cd "$(dirname "$0")" && pwd); S=$1; R=/home/ricky/personal_repos/rs_cam; k=$2; shift 2
cd $R; before=$(git diff | sha256sum); A=crates/rs_cam_core/src/adaptive3d
for f in level_contour clearing left_stock; do command cp -f $A/$f.rs $S/keep_$f.rs; done
python3 $H/revert.py $k || { echo "revert failed"; exit 2; }
git diff --stat -- $A | tail -1
$S/waitmem.sh || exit 3
timeout 3000 scripts/cargo_lane.sh test -p rs_cam_core -q --lib -j 2 -- "$@" > $S/red_$k.log 2>&1; echo "rc $?" >> $S/red_$k.log
for f in level_contour clearing left_stock; do command cp -f $S/keep_$f.rs $A/$f.rs; done
after=$(git diff | sha256sum); [ "$before" = "$after" ] && echo "tree restored" || echo "TREE DIFFERS"
echo "== hunk $k reverted"; grep -a "^test \|test result\|^rc \|panicked\|^error" -A1 $S/red_$k.log | grep -av "^--$\|note: run with" | cut -c1-420 | head -40
