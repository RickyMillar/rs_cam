#!/usr/bin/env bash
# One cargo job at a time on this machine. Usage: cargo_lane.sh <cargo args...>
# Waits for the lane lock, then for any foreign cargo/rustc PROCESS (by name,
# not by command line: pgrep -f matches shells that merely mention cargo) and
# for >= 12 GB available memory, then runs cargo with the given args.
set -u
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)" || exit 2
exec 9>/tmp/rs_cam_cargo_lane.lock
flock 9
while pgrep -x cargo >/dev/null || pgrep -x rustc >/dev/null \
      || [ "$(free -g | awk '/Mem:/{print $7}')" -lt 12 ]; do
  sleep 15
done
nice -n 5 cargo "$@"
