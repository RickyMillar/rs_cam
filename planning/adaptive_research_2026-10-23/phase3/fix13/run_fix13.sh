#!/usr/bin/env bash
# D23 step 13: the fixtures on the D23 build (fixture worktree at 3cc257c1f +
# d23/tree_vs_3cc257c1f.patch), RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug,
# --sim-artifact, as fix7 otherwise. Args: <corne|wanaka200> <arm> <out subdir>.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$R/planning/adaptive_research_2026-10-23/phase3/fix13
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 3cc257c1f ] && git diff | cmp -s - $R/planning/adaptive_research_2026-10-23/phase3/d23/applied.diff && [ -f crates/rs_cam_core/src/adaptive3d/research_outer_band.rs ] && echo "tree = 3cc257c1f + D23 patch" || { echo "TREE NOT AS EXPECTED"; exit 1; }
fx=$1; arm=$2; d=$F/$3; rm -rf $d; mkdir -p $d; s=$(date +%s)
case $fx in corne) proj=$C; extra="--setup 0 --skip 4";; wanaka200) proj=$W; extra="--setup 0 --resolution 1.0";; esac
RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$fx $arm rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
df -h / | tail -1 | awk '{print "  disk free",$4}'
echo RUNDONE
