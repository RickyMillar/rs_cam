#!/usr/bin/env bash
# D68: one product run of the fix41 (D60) job with the rest edge op, on the release binary of the fixture worktree as it
# stands (HEAD + the prototype diff, recorded), and its analysis; the sim is deleted at the end. A copy of fix46/run_one.sh
# (job mode only). Args: <label> <corne|wanaka>.
set -u
R=/home/ricky/personal_repos/rs_cam; WT=$R/.claude/worktrees/fixture-cdc9814e; P=$R/planning/adaptive_research_2026-10-23/phase3; F=$P/d68
label=$1; part=$2; cd $WT
case $part in holes) dpp=6.0; o1="3D Rough 1"; oe="3D Rough 1 [edges, CP rest]"; ke=5; rap=4000; job=$P/d68/jobs/corne_holes_silhouette0.toml; extra="--setup 0 --skip 4";;
            corne) dpp=6.0; o1="3D Rough 1"; oe="3D Rough 1 [edges, CP rest]"; ke=5; rap=4000; job=$P/fix41/jobs/corne_B_s2p1_product_edges.toml; extra="--setup 0 --skip 4";;
            wanaka) dpp=5.46; o1="2 Back Rough"; oe="2 Back Rough [edges, CP rest]"; ke=7; rap=10000; job=$P/fix41/jobs/wanaka_s2p0_product_edges.toml; extra="--setup 0 --resolution 0.6";; esac
s=${label}_$part; d=$F/$s; rm -rf $d; mkdir -p $d
{ git rev-parse HEAD; git diff | md5sum; git diff --stat; } > $d/tree.txt
t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RUST_LOG=info,rs_cam_core::adaptive3d::path=debug timeout 580 target/release/rs_cam_cli project $job --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1); refusals $(grep -ciE refused $d/stderr.txt)"
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt; cd $F
python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
python3 -I depth_by_level.py $d $ke $dpp $dpp > /dev/null 2>depth_$s.err
python3 -I ../fix26/three_clocks.py d68/$s $ke "$oe" $rap > clocks_${s}_tp$ke.json 2>&1
python3 -I ../fix26/three_clocks.py d68/$s 1 "$o1" $rap > clocks_${s}_tp1.json 2>&1
python3 -I entries.py $d ${s}_tp$ke $dpp $part $ke > /dev/null 2>entries_${s}_tp$ke.err
[ $part != wanaka ] && { python3 -I corne_op5.py d68/$s $s > /dev/null 2>left_$s.err; python3 -I corne_reach.py $d/program.nc > reach_$s.txt 2>&1; } || python3 -I op7_wanaka.py d68/$s $s 7 > /dev/null 2>left_$s.err
[ -n "${KEEP_SIM:-}" ] || rm -f $d/simulation.json; echo "  analysed"
