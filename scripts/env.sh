#!/usr/bin/env bash
# Source this helper from build scripts. A local unpacked toolchain is optional.
hardtime_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${HARDTIME_BUILD_ENV:-}" ]]; then
  source "$HARDTIME_BUILD_ENV"
fi
hardtime_work="${HARDTIME_WORK_DIR:-$hardtime_repo/.work}"
mkdir -p -- "$hardtime_work"
hardtime_work="$(cd -- "$hardtime_work" && pwd)"
export HARDTIME_WORK_DIR="$hardtime_work"
export blitzpath="$hardtime_work/release"
