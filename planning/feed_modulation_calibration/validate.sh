#!/usr/bin/env bash
# Pre-flight G-code validation via Docker CAMotics 1.2.0.
#
# Usage:
#   ./validate.sh                 # validates every .nc in ./gcode/
#   ./validate.sh path/to/x.nc    # validates one file
#
# Returns 0 if all files clean, 1 if any flagged.
#
# Setup (one-time):
#   The image `camotics-validator:1.2.0` was built earlier from a
#   Dockerfile at /tmp/gcode_validator/ (Ubuntu 18.04 base + the 2019
#   camotics_1.2.0_amd64.deb). If `docker images` doesn't list it,
#   rebuild via that Dockerfile or ask Claude to recreate it.
#
# The validator runs the full CAMotics interpreter (NOT lexical-only)
# so it catches arc-radius mismatches, motion outside soft limits, and
# the same class of errors GRBL / gSender flag at runtime. Threshold
# `--max-arc-error 0.01` mirrors GRBL's default `$12=0.010`.

set -euo pipefail

IMAGE="camotics-validator:1.2.0"
THRESHOLD="0.01"

if ! docker images "$IMAGE" --format '{{.Repository}}' | grep -q .; then
    echo "ERROR: docker image '$IMAGE' missing." >&2
    echo "Rebuild from /tmp/gcode_validator/Dockerfile or run:" >&2
    echo "  docker build -t $IMAGE /tmp/gcode_validator/" >&2
    exit 2
fi

validate_one() {
    local file="$1"
    local dir base errfile
    dir="$(cd "$(dirname "$file")" && pwd)"
    base="$(basename "$file")"
    errfile="$(mktemp)"

    docker run --rm --entrypoint /usr/bin/gcodetool -v "$dir":/work:ro "$IMAGE" \
        --max-arc-error "$THRESHOLD" --out=- "/work/$base" \
        > /dev/null 2> "$errfile"

    if [[ -s "$errfile" ]]; then
        echo "  FAIL: $base"
        sed 's/^/    /' "$errfile"
        rm -f "$errfile"
        return 1
    else
        echo "  OK:   $base"
        rm -f "$errfile"
        return 0
    fi
}

cd "$(dirname "$0")"

declare -a files
if [[ $# -gt 0 ]]; then
    files=("$@")
else
    mapfile -t files < <(find ./gcode -maxdepth 1 -name '*.nc' | sort)
fi

if [[ ${#files[@]} -eq 0 ]]; then
    echo "No .nc files to validate." >&2
    exit 0
fi

echo "Validating ${#files[@]} file(s) against CAMotics (arc tolerance ${THRESHOLD} mm)..."
fails=0
for f in "${files[@]}"; do
    validate_one "$f" || fails=$((fails+1))
done

if [[ $fails -gt 0 ]]; then
    echo
    echo "$fails file(s) flagged. DO NOT send to the machine until fixed." >&2
    exit 1
fi

echo
echo "All clean. Safe to send to the machine."
exit 0
