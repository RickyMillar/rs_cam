#!/usr/bin/env bash
# D20b (lead-approved option b): the CLI's island list with the advisory cap
# raised to 100000 in the fixture worktree only, at f32401326 (D18) and
# b37cad1a4 (D20); program.nc must be byte-equal to fix5 / fix7; the file is
# reverted with git checkout and the binary rebuilt clean at b37cad1a4.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
P=$R/planning/adaptive_research_2026-10-23/phase3
F=$P/fix7/d20b
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
T=crates/rs_cam_core/src/stock/sim_triage.rs
cd $WT
one() { commit=$1; tag=$2; ref=$3
  git checkout -q --detach $commit || { echo "checkout $commit failed"; exit 1; }
  grep -n "pub const ADVISORY_CAP_PER_TOOLPATH: usize = 10;" $T || { echo "cap line not found"; exit 1; }
  sed -i 's/pub const ADVISORY_CAP_PER_TOOLPATH: usize = 10;/pub const ADVISORY_CAP_PER_TOOLPATH: usize = 100000;/' $T
  echo "$tag edit: $(git diff --stat | tail -1)"
  timeout 1800 scripts/cargo_lane.sh build --release -q -p rs_cam_cli --features rs_cam_core/research > $F/build_$tag.log 2>&1; echo "$tag build rc=$?"
  d=$F/$tag; rm -rf $d; mkdir -p $d; s=$(date +%s)
  RUST_LOG=info RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_zigzag_pick timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $C --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
  echo "$tag run rc=$? $(( $(date +%s)-s ))s"
  cmp -s $d/program.nc $ref/program.nc && echo "$tag program.nc byte-equal to $ref" || echo "$tag program.nc DIFFERS from $ref"
  rm -f $d/simulation.json
  git checkout -- $T; echo "$tag reverted: [$(git status --short)]"; }
mkdir -p $F
one f32401326 d18 $P/fix5/corne/geodesic_local_zigzag_pick
one b37cad1a4 d20 $P/fix7/corne/geodesic_local_zigzag_pick
timeout 1800 scripts/cargo_lane.sh build --release -q -p rs_cam_cli --features rs_cam_core/research > $F/build_clean.log 2>&1; echo "clean rebuild rc=$? at $(git log --oneline -1 | cut -c1-9)"
echo "git status --short: [$(git status --short)]"
strings target/release/rs_cam_cli | grep -o '0\.1\.0[0-9a-f]\{9\}' | head -1
echo D20BDONE
