#!/usr/bin/env bash
# C43 (fix24): the load oracle on contour_parallel (fix6), D26 (fix16), C35 (fix12) and D33 (fix23 default + stock_depth control), op 1, both fixtures. Python only.
set -euo pipefail
cd "$(dirname "$0")"
for r in "fix6/corne/contour_parallel corne" "fix16/corne_pick corne" "fix12/corne corne" "fix23/default/corne corne" "fix23/stock_depth/corne corne" \
         "fix6/wanaka200/contour_parallel wanaka200" "fix16/wanaka200_pick wanaka200" "fix12/wanaka200 wanaka200" "fix23/default/wanaka200 wanaka200" "fix23/stock_depth/wanaka200 wanaka200"; do
  python3 -I area_oracle.py $r
done
python3 -I rescore.py
python3 -I synthetic.py
python3 -I make_readme.py || true

