#!/bin/bash
# usage: run_cli.sh <cli tag> <out tag> [setup] [job file]; runs cli_<tag> into out_<out tag> ("1 Back") or outF_<out tag> ("3 Front")
S=$(cd "$(dirname "$0")" && pwd); cd $S; t=$1; o=$2; setup=${3:-1 Back}; job=${4:-$S/job_no41.toml}
avail=$(command free -g | awk '/Mem:/{print $7}' | tr -dc 0-9); if [ "$avail" -lt 18 ]; then echo "only $avail GB available"; exit 3; fi
pre=out; [ "$setup" = "3 Front" ] && pre=outF
RUST_LOG=${RUST_LOG:-info} /usr/bin/time -v timeout 2400 ./cli_$t project $job --setup "$setup" --output-dir $S/${pre}_$o --emit-gcode $S/${pre}_$o.nc --summary > run${pre#out}_$o.log 2>&1
echo "cli rc $?"; grep -a "Maximum resident\|Elapsed (wall" run${pre#out}_$o.log
