#!/usr/bin/env bash
# C35: build fixture3d_v3.html ("Geodesic roughing G2") and assert every number on it.
set -euo pipefail
cd "$(dirname "$0")"
python3 -I fixture3d_v3.py spec_v3.json
python3 -I assert_fixture3d_v3.py spec_v3.json
