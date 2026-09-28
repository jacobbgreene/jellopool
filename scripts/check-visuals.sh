#!/usr/bin/env bash
# Render deterministic smoke scenes; retain PNGs/logs for human visual review.
set -euo pipefail

if (( $# > 1 )); then
    echo "Usage: bash scripts/check-visuals.sh [new-output-directory]" >&2
    exit 2
fi

# Resolve paths before changing directory. Refuse to overwrite an earlier run.
visual_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
if (( $# == 1 )); then
    mkdir -- "$1"
    visual_output=$(cd -- "$1" && pwd -P)
else
    visual_output=$(mktemp -d "${TMPDIR:-/tmp}/jellopool-visuals-XXXXXX")
fi
echo "Visual artifacts: $visual_output"
cd -- "$visual_repo"

# Pin scene content/fonts, but leave graphics-driver selection to the caller.
unset JELLOPOOL_SEED JELLOPOOL_CAPTURE_FONT JELLOPOOL_CAPTURE_NO_SIGNATURE JELLOPOOL_WINDOWED
export BEVY_ASSET_ROOT="$visual_repo"
cargo build --locked --bin poetry_game

capture_scene() {
    local scene=$1 size=$2 scale=$3 label=$4
    echo "Checking $label ($size @ $scale)..."
    if ! JELLOPOOL_SCENE="$scene" \
        JELLOPOOL_CAPTURE_SIZE="$size" \
        JELLOPOOL_SCALE_FACTOR="$scale" \
        JELLOPOOL_CAPTURE="$visual_output/$label.png" \
        cargo run --locked --quiet --bin poetry_game >"$visual_output/$label.log" 2>&1; then
        echo "Visual check failed: $label. Log: $visual_output/$label.log" >&2
        cat -- "$visual_output/$label.log" >&2
        return 1
    fi
    if [[ ! -s "$visual_output/$label.png" ]]; then
        echo "No screenshot produced: $label" >&2
        return 1
    fi
}

for scene in empty poem dense scattered edge scrolled push phase; do
    capture_scene "$scene" 1600x900 1.0 "$scene"
done
capture_scene phase 1280x720 1.5 phase-scaled

echo "All 9 rendered checks passed. Review PNGs in $visual_output"
