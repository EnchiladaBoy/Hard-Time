#!/usr/bin/env python3
"""Create a private native installation from user-provided extracted assets."""
from pathlib import Path
import os
import shutil
import sys

binary, assets, destination = (Path(p).expanduser().resolve() for p in sys.argv[1:])
asset_directories = ("Characters", "Graphics", "Sound", "Weapons", "World")
if not binary.is_file():
    raise SystemExit(f"Build the game first; missing {binary}")
if destination == assets or assets in destination.parents or destination in assets.parents:
    raise SystemExit("Use an installation directory separate from the supplied original game.")
for name in (*asset_directories, "Data"):
    if not (assets / name).is_dir():
        raise SystemExit(f"Missing extracted game directory: {assets / name}")
for name in ("Data", "Photo Album"):
    target = destination / name
    if target.is_symlink() or (target.exists() and not target.is_dir()):
        raise SystemExit(f"Private writable directory required: {target}")
destination.mkdir(parents=True, exist_ok=True)
lock_path = destination / ".launch.lock"
import fcntl
with lock_path.open("a") as lock:
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        raise SystemExit("Close this native installation before updating it.")
    for name in asset_directories:
        target = destination / name
        if target.exists() or target.is_symlink():
            if target.resolve() != (assets / name).resolve():
                raise SystemExit(f"Existing assets differ; refusing to replace {target}")
        else:
            target.symlink_to(assets / name, target_is_directory=True)
    data = destination / "Data"
    if not data.exists():
        shutil.copytree(assets / "Data", data, symlinks=False)
    (destination / "Photo Album").mkdir(exist_ok=True)
    temporary = destination / ".hardtime-native.new"
    shutil.copy2(binary, temporary)
    temporary.chmod(0o755)
    temporary.replace(destination / "hardtime-native")
    launcher = destination / "play.sh"
    launcher.write_text('''#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
[[ -d "$root/Data" && ! -L "$root/Data" ]] || { echo 'Private Data directory required.' >&2; exit 1; }
exec 9>"$root/.launch.lock"
flock -n 9 || { echo 'Hard Time is already running.' >&2; exit 0; }
mkdir -p -- "$root/backups" "$root/logs"
stamp="$(date +%Y%m%d-%H%M%S-%N)"
cp -a -- "$root/Data" "$root/backups/before-launch-$stamp"
log="$root/logs/run-$stamp.log"
ln -sfn -- "$(basename -- "$log")" "$root/logs/latest.log"
export SDL_VIDEODRIVER="${SDL_VIDEODRIVER:-x11}"
cd -- "$root"
set +e
./hardtime-native >>"$log" 2>&1
status=$?
set -e
printf 'Exit status: %s\\n' "$status" >>"$log"
if (( status )); then printf 'Hard Time exited with status %s. See %s\\n' "$status" "$log" >&2; fi
exit "$status"
''')
    launcher.chmod(0o755)
print(f"Launch {destination / 'play.sh'}")
print("Existing private Data and Photo Album directories were preserved.")
