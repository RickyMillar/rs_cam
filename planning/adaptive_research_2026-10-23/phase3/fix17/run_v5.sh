#!/usr/bin/env bash
# C41 (fix17): rebuild fixture3d_v5.html and check it. Python only, no cargo.
set -euo pipefail
cd "$(dirname "$0")"
python3 -I stock_v5.py
python3 -I entries_v5.py
python3 -I render_v5.py
python3 -I numbers_v5.py
python3 -I fixture3d_v5.py
python3 -I assert_fixture3d_v5.py
