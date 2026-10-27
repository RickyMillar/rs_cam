#!/usr/bin/env bash
# D64: one op-1 strategy run (Corne: --skip 3,4, so the program holds op 1 only, as on wanaka where the edge op is disabled) on geodesic-integration 7ee3d3226 (release binary run directly; no research feature; no env;
# no override), then the analysis; the simulation.json is deleted at the end. Args: <corne_B_s2p1|wanaka_s2p0> <strategy>.
set -u
R=/home/ricky/personal_repos/rs_cam; WT=$R/.claude/worktrees/fixture-cdc9814e; F=$R/planning/adaptive_research_2026-10-23/phase3/fix45
cd $WT; [ "$(git rev-parse --short=9 HEAD)" = 7ee3d3226 ] && [ -z "$(git status --short)" ] || { echo "TREE NOT AS EXPECTED"; exit 1; }
p=$1; st=$2; s=${p}_$st; d=$F/$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
case $p in corne*) extra="--setup 0 --skip 3,4"; o1="3D Rough 1"; rap=4000; part=corne; mrr=182.25; dpp=6.0;;
           wanaka*) extra="--setup 0 --resolution 0.6"; o1="2 Back Rough"; rap=10000; part=wanaka200; mrr=182.0; dpp=5.46;; esac
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RUST_LOG=info timeout 560 target/release/rs_cam_cli project $F/jobs/$s.toml --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
rc=$?; echo "$s rc=$rc $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1); refusals $(grep -ciE refused $d/stderr.txt)"
grep -iE "refused|error" $d/stderr.txt | head -3 | sed 's/^/  /'
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
cd $F; [ -f $d/simulation.json ] || { echo "  no simulation.json"; exit 0; }
python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
[ -f $d/program.nc ] && python3 -I ../fix26/three_clocks.py fix45/$s 1 "$o1" $rap > clocks_${s}_tp1.json 2>&1
[ -f $d/program.nc ] && python3 -I ../fix25/audit_d34.py ../fix45/$s --op "$o1" --tp "tp_1_*.json" --nominal-mrr $mrr --json audit_${s}_tp1.json > audit_${s}_tp1.txt 2>&1
AREA_SCREEN=0 python3 -I area_oracle.py fix45/$s $part 1 > area_$s.log 2>&1
python3 -I depth_by_level.py $d 1 $dpp $dpp > /dev/null 2>depth_$s.err
case $p in corne*) python3 -I corne_left.py $d $s > /dev/null 2>left_$s.err;; wanaka*) python3 -I op7_wanaka.py fix45/$s $s 0 > /dev/null 2>left_$s.err;; esac
rm -f $d/simulation.json; echo "  analysed; sim deleted"
