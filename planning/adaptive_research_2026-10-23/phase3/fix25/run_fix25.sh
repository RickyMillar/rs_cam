#!/usr/bin/env bash
# D34 fixtures: fixture worktree at 3181ae54f + d34/tree_vs_3181ae54f.patch
# (+ slab_field.rs, a_feed_cap test). RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug,
# --sim-artifact, the D27 flags. RES=<mm> sets the wanaka200 resolution (default 1.0;
# the body_cp job needs 0.6 or finer: G-RESTRES, the rest tool). Args: <col: body_cp|body_cp_field|cp_alone|cp_alone_0.6> <corne|wanaka200>.
#  body_cp:       the body_cp_edges job, arm geodesic_local_spiral_pick, field off
#  body_cp_field: the same with RS_CAM_RESEARCH_SLAB_FIELD=on
#  cp_alone:      phase0/projects/<part>_contour_parallel.toml, no research arm variable
#                 (wanaka200: --skip 0 drops the Pin Drill, as the rough-only control of fix6)
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix25
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 3181ae54f ] && git diff | cmp -s - $A/phase3/d34/applied.diff && [ -f crates/rs_cam_core/src/adaptive/slab_field.rs ] && echo "tree = 3181ae54f + D34" || { echo "TREE NOT AS EXPECTED"; exit 1; }
col=$1; fx=$2; d=$F/$col/$fx; rm -rf $d; mkdir -p $d; s=$(date +%s)
case $col in
  body_cp|body_cp_field) case $fx in corne) proj=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_body_cp_edges.toml;; wanaka200) proj=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only_body_cp_edges.toml;; esac
                         envs="RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick"; [ $col = body_cp_field ] && envs="$envs RS_CAM_RESEARCH_SLAB_FIELD=on";;
  cp_alone|cp_alone_0.6) proj=$A/phase0/projects/${fx}_contour_parallel.toml; envs="";;
esac
case $fx in corne) extra="--setup 0 --skip 4";; wanaka200) extra="--setup 0 --resolution ${RES:-1.0}"; [ ${col%_0.6} = cp_alone ] && extra="$extra --skip 0";; esac
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD $envs RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$col $fx rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
df -h / | tail -1 | awk '{print "  disk free",$4}'
