#!/usr/bin/env bash
# D66: one product run (release binary of the checked-out tree, run directly; no env; no override) and its analysis; the
# sim is deleted at the end. Args: <tree> <corne|wanaka> <op1|job>. op1: fix45's geodesic job (Corne --skip 3,4; wanaka edge
# op disabled). job: fix41's job with the rest-region edge op (Corne --skip 4).
set -u
R=/home/ricky/personal_repos/rs_cam; WT=$R/.claude/worktrees/fixture-cdc9814e; P=$R/planning/adaptive_research_2026-10-23/phase3; F=$P/fix46
tree=$1; part=$2; mode=$3; cd $WT; [ "$(git rev-parse --short=9 HEAD)" = $tree ] && [ -z "$(git status --short)" ] || { echo "TREE NOT AS EXPECTED"; exit 1; }
case $part in corne) dpp=6.0; o1="3D Rough 1"; oe="3D Rough 1 [edges, CP rest]"; ke=5; rap=4000; ap=corne; mrr=182.25
                   [ $mode = op1 ] && { job=$P/fix45/jobs/corne_B_s2p1_geodesic_adaptive.toml; extra="--setup 0 --skip 3,4"; } || { job=$P/fix41/jobs/corne_B_s2p1_product_edges.toml; extra="--setup 0 --skip 4"; };;
            wanaka) dpp=5.46; o1="2 Back Rough"; oe="2 Back Rough [edges, CP rest]"; ke=7; rap=10000; ap=wanaka200; mrr=182.0; extra="--setup 0 --resolution 0.6"
                   [ $mode = op1 ] && job=$P/fix45/jobs/wanaka_s2p0_geodesic_adaptive.toml || job=$P/fix41/jobs/wanaka_s2p0_product_edges.toml;; esac
s=${tree}_${part}_$mode; d=$F/$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RUST_LOG=info timeout 560 target/release/rs_cam_cli project $job --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1); refusals $(grep -ciE refused $d/stderr.txt)"
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt; cd $F
python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
python3 -I ../fix26/three_clocks.py fix46/$s 1 "$o1" $rap > clocks_${s}_tp1.json 2>&1
python3 -I ../fix25/audit_d34.py ../fix46/$s --op "$o1" --tp "tp_1_*.json" --nominal-mrr $mrr --json audit_${s}_tp1.json > audit_${s}_tp1.txt 2>&1
AREA_SCREEN=0 python3 -I area_oracle.py fix46/$s $ap 1 > area_$s.log 2>&1
python3 -I entries.py $d $s $dpp $part > /dev/null 2>entries_$s.err
if [ $part = corne ]; then python3 -I cap_audit.py $d $s 0,-109.31554412841797 0.05 > /dev/null 2>cap_$s.err; python3 -I cap_check_offmap.py $d 0,-109.31554412841797 "$o1" > cap_lineK_$s.txt 2>&1
else python3 -I cap_audit.py $d $s > /dev/null 2>cap_$s.err; fi
if [ $mode = op1 ]; then python3 -I depth_by_level.py $d 1 $dpp $dpp > /dev/null 2>depth_$s.err
  [ $part = corne ] && python3 -I corne_left.py $d $s > /dev/null 2>left_$s.err || python3 -I op7_wanaka.py fix46/$s $s 0 > /dev/null 2>left_$s.err
else python3 -I depth_by_level.py $d $ke $dpp $dpp > /dev/null 2>depth_$s.err
  python3 -I ../fix26/three_clocks.py fix46/$s $ke "$oe" $rap > clocks_${s}_tp$ke.json 2>&1
  [ $part = corne ] && { python3 -I corne_op5.py fix46/$s $s > /dev/null 2>left_$s.err; python3 -I corne_reach.py $d/program.nc > reach_$s.txt 2>&1; } || python3 -I op7_wanaka.py fix46/$s $s 7 > /dev/null 2>left_$s.err
fi
rm -f $d/simulation.json; echo "  analysed; sim deleted"
