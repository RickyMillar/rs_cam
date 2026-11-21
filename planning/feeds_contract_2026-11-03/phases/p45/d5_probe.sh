#!/bin/bash
# P45, D-5: the lib parity fixture with the fresh wall pass on (a temporary edit of the test helper; restored after).
# usage: d5_probe.sh <scratch dir>
S=$1; R=/home/ricky/personal_repos/rs_cam; cd $R; T=crates/rs_cam_core/src/adaptive3d/tests.rs
before=$(git diff | sha256sum); command cp -f $T $S/keep_tests.rs
python3 - <<'PY'
f='/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/adaptive3d/tests.rs'
s=open(f).read()
old="        p.geometry.world_stock_xy_bbox = world_xy;\n        p\n"
assert s.count(old)==1
s=s.replace(old,"        p.geometry.world_stock_xy_bbox = world_xy;\n        if std::env::var(\"P45_D5\").is_ok() {\n            p.linking.one_pass_hold = OnePassHold::FreshStock {\n                link_kinematics: None,\n            };\n        }\n        p\n")
open(f,'w').write(s)
PY
P45_D5=1 RS_CAM_PARITY_MAP=1 timeout 3000 scripts/cargo_lane.sh test -p rs_cam_core --lib -j 2 -- --nocapture planner_sim_dexel_parity_geodesic_adaptive_world_stock_declared > $S/d5_on.log 2>&1; echo "rc $?" >> $S/d5_on.log
RS_CAM_PARITY_MAP=1 timeout 3000 scripts/cargo_lane.sh test -p rs_cam_core --lib -j 2 -- --nocapture planner_sim_dexel_parity_geodesic_adaptive_world_stock_declared > $S/d5_off.log 2>&1; echo "rc $?" >> $S/d5_off.log
command cp -f $S/keep_tests.rs $T
after=$(git diff | sha256sum); [ "$before" = "$after" ] && echo "tree restored" || echo "TREE DIFFERS"
grep -a "PARITY\|W5B\|^rc\|test result\|panicked" $S/d5_on.log $S/d5_off.log | cut -c1-400
