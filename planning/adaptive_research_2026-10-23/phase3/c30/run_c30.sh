#!/usr/bin/env bash
# C30: attribute the '3D Rough 1' samples over the load bounds to producers.
# Run from any directory; writes rows_*.pkl / exact_*.pkl beside the scripts.
set -euo pipefail
cd "$(dirname "$0")"
P=..
run1() { python3 -I attribute.py "$1" "$2"; python3 -I exact.py "$1" "$2"; python3 -I report.py "$2"; }
run1 $P/fix5/corne/geodesic_local_zigzag_pick zz_pick
run1 $P/fix4/corne/geodesic_local_zigzag zigzag
run1 $P/fix4/corne/geodesic_local_oneway oneway
run1 $P/fix2/simart/corne/agent agent
