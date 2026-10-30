#!/usr/bin/env bash
# Cargo jobs on this machine, at most CARGO_LANE_SLOTS (default 2) at once.
# Usage: cargo_lane.sh <cargo args...>
# Takes a free slot lock, then waits for >= 12 GB available memory, then runs
# cargo with the given args. Two jobs on the same target directory still
# compile one at a time: cargo holds its own build-directory lock. A second
# slot lets a test run or a check go while another job compiles.
# Slot 1 keeps the old lock file, so a job of the one-slot script still
# blocks it.
set -u
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)" || exit 2
slots="${CARGO_LANE_SLOTS:-2}"
got=0
while [ "$got" = 0 ]; do
  for i in $(seq 1 "$slots"); do
    if [ "$i" = 1 ]; then lock=/tmp/rs_cam_cargo_lane.lock; else lock="/tmp/rs_cam_cargo_lane.$i.lock"; fi
    exec 9>"$lock"
    if flock -n 9; then got=1; break; fi
    exec 9>&-
  done
  [ "$got" = 1 ] || sleep 5
done
while [ "$(free -g | awk '/Mem:/{print $7}')" -lt 12 ]; do
  sleep 15
done
nice -n 5 cargo "$@"
