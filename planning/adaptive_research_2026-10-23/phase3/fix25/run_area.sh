#!/usr/bin/env bash
# D34: the area oracle (fix25/area_oracle.py, a copy of C18's fix24 script) per op.
cd "$(dirname "$0")"
for r in "fix25/body_cp/corne corne 1" "fix25/body_cp/corne corne 5" "fix25/body_cp_field/corne corne 1" "fix25/body_cp_field/corne corne 5" "fix25/cp_alone/corne corne 1" \
         "fix25/body_cp/wanaka200 wanaka200 1" "fix25/body_cp/wanaka200 wanaka200 7" "fix25/body_cp_field/wanaka200 wanaka200 1" "fix25/body_cp_field/wanaka200 wanaka200 7" "fix25/cp_alone_0.6/wanaka200 wanaka200 1" "fix25/cp_alone/wanaka200 wanaka200 1"; do
  python3 -I area_oracle.py $r
done
echo AREADONE
