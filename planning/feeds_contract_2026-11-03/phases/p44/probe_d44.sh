#!/bin/bash
# usage: probe_d44.sh <H1|H2|NONE>: the D44 sentry with a print of each published rest region, with one hunk reverted. Restores the tree.
S=$(cd "$(dirname "$0")" && pwd); R=/home/ricky/personal_repos/rs_cam; k=$1; cd $R
T=crates/rs_cam_core/tests/a_derived_rest_rough_cuts_only_the_leftovers_d44.rs
before=$(git diff -- crates | sha256sum)
cp crates/rs_cam_core/src/adaptive3d/clearing.rs $S/keep_clearing.rs; cp crates/rs_cam_core/src/adaptive3d/left_stock.rs $S/keep_left_stock.rs; cp $T $S/keep_d44.rs
[ "$k" = NONE ] || python3 $S/revert.py $k || { echo "revert failed"; exit 2; }
python3 - <<'PY'
p='crates/rs_cam_core/tests/a_derived_rest_rough_cuts_only_the_leftovers_d44.rs'; s=open(p).read()
a='''    let missed: Vec<P2> = channel_points()'''
b='''    for r in &regions {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for p in &r.exterior {
            x0 = x0.min(p.x); x1 = x1.max(p.x); y0 = y0.min(p.y); y1 = y1.max(p.y);
        }
        eprintln!("PROBE region area {:.1} box x {x0:.2}..{x1:.2} y {y0:.2}..{y1:.2}", r.area().abs());
    }
    let missed: Vec<P2> = channel_points()'''
assert s.count(a)==1; open(p,'w').write(s.replace(a,b))
PY
timeout 2400 scripts/cargo_lane.sh test -p rs_cam_core -q --features test-support,heavy-tests --test a_derived_rest_rough_cuts_only_the_leftovers_d44 -j 2 -- --nocapture > $S/probe_d44_$k.log 2>&1; echo "rc $?" >> $S/probe_d44_$k.log
cp $S/keep_clearing.rs crates/rs_cam_core/src/adaptive3d/clearing.rs; cp $S/keep_left_stock.rs crates/rs_cam_core/src/adaptive3d/left_stock.rs; cp $S/keep_d44.rs $T
[ "$before" = "$(git diff -- crates | sha256sum)" ] && echo "tree restored" || echo "TREE DIFFERS"
echo "== $k"; grep -a "PROBE\|rough:\|^rc\|test result\|the stated area" $S/probe_d44_$k.log | cut -c1-300
