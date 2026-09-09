#!/bin/bash
# Serialised cargo runner for the shared target dir.
# usage: /tmp/rs_cam_gate.sh <tree-dir> cargo <args...>
# Cargo keys workspace crates WITHOUT the source path, so two worktrees
# overwrite each other's artefacts. This runner takes a global lock and,
# when the tree differs from the one that last built, touches every .rs
# under crates/ so cargo rebuilds the workspace crates from THIS tree.
set -u
tree="$1"; shift
export CARGO_TARGET_DIR=/home/ricky/personal_repos/rs_cam/target
exec flock /tmp/rs_cam_cargo.lock bash -c '
  tree="$1"; shift
  cd "$tree" || exit 1
  last=$(cat /tmp/rs_cam_last_tree 2>/dev/null || true)
  if [ "$last" != "$tree" ]; then
    find crates -name "*.rs" -exec touch {} +
    printf "%s" "$tree" > /tmp/rs_cam_last_tree
    echo "[gate] tree switched from ${last:-none} -> $tree: workspace crates touched" >&2
  fi
  "$@"
' _ "$tree" "$@"
