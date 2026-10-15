#!/usr/bin/env bash
set -euo pipefail
if (( $# != 2 )); then
  printf 'Usage: scripts/install.sh /path/to/extracted-game /path/to/private-install\n' >&2
  exit 2
fi
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/env.sh"
python3 "$script_dir/install.py" "$hardtime_work/game/hardtime-native" "$1" "$2"
