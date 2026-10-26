#!/usr/bin/env bash
# D30 fixtures: fixture worktree at 3181ae54f + d30/tree_vs_3181ae54f.patch
# (+ the new test file a_feed_cap_binds_every_feed_stage_c38.rs), spiral_pick, RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug,
# --sim-artifact, the D27 flags. Args: <col: a_only|cut_feed> <route value> <corne|wanaka200>.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$R/planning/adaptive_research_2026-10-23/phase3/fix21
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 3181ae54f ] && git diff | cmp -s - $R/planning/adaptive_research_2026-10-23/phase3/d30/applied.diff && [ -f crates/rs_cam_core/tests/a_feed_cap_binds_every_feed_stage_c38.rs ] && echo "tree = 3181ae54f + D30" || { echo "TREE NOT AS EXPECTED"; exit 1; }
col=$1; route=$2; fx=$3; d=$F/$col/$fx; rm -rf $d; mkdir -p $d; s=$(date +%s)
case $fx in corne) proj=$C; extra="--setup 0 --skip 4";; wanaka200) proj=$W; extra="--setup 0 --resolution 1.0";; esac
RS_CAM_RESEARCH_KEEP_DOWN_ROUTE=$route RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$col $fx route=$route rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
df -h / | tail -1 | awk '{print "  disk free",$4}'
