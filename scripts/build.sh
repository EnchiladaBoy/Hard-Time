#!/usr/bin/env bash
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/env.sh"
case "$(uname -m)" in aarch64|arm64) ;; *) printf 'This build currently supports native ARM64 Linux only.\n' >&2; exit 1;; esac
[[ "$(uname -s)" == Linux ]] || { printf 'Linux is required.\n' >&2; exit 1; }
for command in cmake ninja gcc g++ python3 git; do
  command -v "$command" >/dev/null || { printf 'Missing build dependency: %s\n' "$command" >&2; exit 1; }
done
python3 "$hardtime_repo/scripts/prepare.py"
cmake_args=(
  -S "$hardtime_work/engine" -B "$hardtime_work/build" -G Ninja
  -DBB_PLATFORM=linux -DARCH=arm64 -DBB_ENV=release
  -DOUTPUT_PATH="$blitzpath" -DCMAKE_POLICY_VERSION_MINIMUM=3.5
  -DCMAKE_C_COMPILER="$(command -v gcc)" -DCMAKE_CXX_COMPILER="$(command -v g++)"
  -DCMAKE_C_STANDARD=11
)
if [[ -n "${LLVM_ROOT:-}" ]]; then
  cmake_args+=(-DLLVM_DIR="$LLVM_ROOT/lib/cmake/llvm" -DLLD_DIR="$LLVM_ROOT/lib/cmake/lld")
elif command -v llvm-config-19 >/dev/null; then
  llvm_cmake="$(llvm-config-19 --cmakedir)"
  cmake_args+=(-DLLVM_DIR="$llvm_cmake" -DLLD_DIR="${llvm_cmake%/llvm}/lld")
else
  printf 'Set LLVM_ROOT to your LLVM 19 prefix, or install llvm-config-19.\n' >&2
  exit 1
fi
cmake "${cmake_args[@]}" "$@"
if [[ "${HARDTIME_CONFIGURE_ONLY:-0}" == 1 ]]; then exit 0; fi
cmake --build "$hardtime_work/build" --target blitzcc runtime.opengl.shared runtime.opengl.static -j "${HARDTIME_BUILD_JOBS:-2}"
cd -- "$hardtime_work/game"
"$blitzpath/bin/blitzcc" -r opengl -o hardtime-native Gameplay.bb
printf 'Built native game: %s/game/hardtime-native\n' "$hardtime_work"
