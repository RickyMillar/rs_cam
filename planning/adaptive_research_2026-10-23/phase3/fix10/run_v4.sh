#!/usr/bin/env bash
# fix10: render the images, build fixture3d_v4.html, assert every number on it.
# Needs phase3/fix9/stock_*.npz and entries.json (fix9/stockmap.py, fix9/entries.py).
set -euo pipefail
cd "$(dirname "$0")"
python3 -I render_v4.py
python3 -I fixture3d_v4.py spec_v4.json
python3 -I assert_fixture3d_v4.py spec_v4.json
