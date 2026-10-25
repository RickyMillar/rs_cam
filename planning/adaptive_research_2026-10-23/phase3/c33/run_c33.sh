#!/usr/bin/env bash
# C33: attribute the thick over-0.471 body load of Corne '3D Rough 1' (D20).
set -euo pipefail
cd "$(dirname "$0")"
python3 -I partmap.py
python3 -I c33.py
python3 -I report33.py | tee c33_output.txt
