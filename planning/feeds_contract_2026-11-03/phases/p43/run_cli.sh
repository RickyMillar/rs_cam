#!/bin/bash
# usage: run_cli.sh <tag> [setup] ; runs cli_<tag>; out_<tag> (1 Back) or outF_<tag> (3 Front)
S=$(cd "$(dirname "$0")" && pwd); cd $S; t=$1; setup=${2:-1 Back}
avail=$(command free -g | awk '/Mem:/{print $7}' | tr -dc 0-9); if [ "$avail" -lt 18 ]; then echo "only $avail GB available"; exit 3; fi
pre=out; [ "$setup" = "3 Front" ] && pre=outF
/usr/bin/time -v timeout 1500 ./cli_$t project /home/ricky/personal_repos/rs_cam/planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml --setup "$setup" --output-dir $S/${pre}_$t --emit-gcode $S/${pre}_$t.nc --summary > run${pre#out}_$t.log 2>&1
echo "cli rc $?"; grep -a "Maximum resident\|Elapsed (wall" run${pre#out}_$t.log
