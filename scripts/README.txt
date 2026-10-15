NATIVE ARM64 BUILD WORKFLOW

Run these commands from the repository root:

  python3 scripts/prepare.py
  scripts/build.sh
  scripts/install.sh /path/to/extracted-original-game "$HOME/Games/Hard-Time-native"

Launch the installed play.sh printed by the last command. Assets remain linked
to the supplied extracted game directory, so keep that directory available.
Data is copied into the new installation only on its first install. Subsequent
installs preserve the destination's Data and Photo Album contents. Progress is
independent of the original game. The launcher backs up Data before each run,
prevents concurrent launches, and stores output in logs/latest.log. It uses
SDL's tested X11 path by default; an existing SDL_VIDEODRIVER override is kept.

SUPPORTED BUILD HOST AND DEPENDENCIES

Native ARM64 Linux with glibc and GCC/G++. This workflow does not cross-compile.
The previous complete game was built on Fedora 44. Other distributions require
their corresponding development packages and have not received full gameplay
validation. The historical binary requires GLIBC_2.43; build from source for
your own system instead of treating that binary as portable across releases.

Required build tools: Python 3, Git, CMake >= 3.24, Ninja, GCC and G++ with glibc
development/startup objects, pkg-config, LLVM 19.1.7 development libraries and
LLD 19 development libraries. LLVM's exported CMake targets must include the
shared LLVM library and lldELF/lldWasm. Set LLVM_ROOT to its installation prefix
or provide llvm-config-19 on PATH. The build uses GCC for the C/C++ engine and
LLVM 19 inside the BASIC compiler.

Required native development libraries include OpenAL, Fontconfig, UUID,
OpenGL, and X11 with Xext, Xrandr, Xinerama, Xcursor, Xxf86vm, Xi, Xfixes and
Xrender. SDL can optionally detect system audio/display backends such as
PulseAudio and Wayland. Install the desired backend development packages.
The pinned engine supplies its own SDL2, FreeType, GLEW, FreeImage, PNG, zlib,
Ogg, Vorbis and MP3 decoder sources. Headers and system library links must
resolve normally through your compiler/CMake environment.

At runtime install Comfortaa and Liberation Sans/Serif/Mono fonts, OpenAL,
Fontconfig, OpenGL and X11/XWayland. OpenGL 3.3 core support is required.
Fedora package names include aajohan-comfortaa-fonts and liberation-fonts.
Install library development packages using your distribution's package manager;
these scripts do not install system packages or request root access.

PATHS AND OVERRIDES

HARDTIME_WORK_DIR defaults to .work/ in this repository. It contains:
  engine/          Pinned engine and dependency checkouts with patches applied.
  source-original/ Hash-verified, unmodified archived game source.
  game/            Patched BASIC source and built hardtime-native executable.
  build/           CMake/Ninja build directory.
  release/         BASIC compiler, runtime, target libraries and metadata.
  prepared.json    Input recipe fingerprint for safe repeated preparation.

The work directory is ignored by Git. Change it to make a separate build.
prepare.py refuses to replace existing source checkouts. If preparation is
interrupted or patch inputs change, choose a fresh HARDTIME_WORK_DIR. Keep any
local source edits before removing an old work directory. Preparation is
idempotent for an unchanged successful recipe; it does not reset local edits.

HARDTIME_BUILD_JOBS defaults to 2. Additional build.sh arguments are forwarded
to CMake, for example -DLIBARCH_DIR=/custom/lib or -DBB_DYNAMIC_LINKER=/path.
The compiler queries GCC for glibc and GCC startup object locations. The
default loader is /lib/ld-linux-aarch64.so.1, validated during configuration.

HARDTIME_CONFIGURE_ONLY=1 scripts/build.sh performs preparation and CMake
configuration without compiling the engine or game.

HARDTIME_BUILD_ENV=/path/to/local-env.sh scripts/build.sh sources an optional
existing toolchain environment. This can provide PATH, LD_LIBRARY_PATH,
GCC_EXEC_PREFIX, CFLAGS, CXXFLAGS, LIBRARY_PATH, CMAKE_PREFIX_PATH and LLVM_ROOT
for locally unpacked build packages. It is optional and is never packaged with
the game. No specific user's path is embedded in the public build recipe.

For offline preparation or faster local validation:
  HARDTIME_ENGINE_REFERENCE=/path/to/existing/engine
  HARDTIME_GAME_SOURCE_REFERENCE=/path/to/original-18-BASIC-files
The first uses local Git object stores while checking out the pinned commits;
working-tree edits are not copied. Keep those referenced object stores alive
while using that work directory. Without these overrides, sources are fetched
from their recorded public upstreams. Every original BASIC file is SHA256
checked in either mode. No game artwork, sound, models or saves are downloaded.

PATCH ORDER AND PROVENANCE

scripts/source-lock.json pins the original source hashes and upstream commits.
Preparation applies engine-linux-arm64.patch, the SDL/Vorbis dependency patches,
portable-build.patch, then game-linux.patch to their appropriate directories.
The engine patch's historical machine-specific library directory was normalized
to /opt/hardtime-toolchain/lib in its intermediate hunk; portable-build.patch
immediately replaces that hunk with compiler-discovered/cache-configurable
paths. The normalized intermediate directory is not a required installation.
The portable patch also discovers UUID through CMake and makes the dependency
Vorbis adjustment idempotent. No Windows x64 work is part of this workflow.

The source checksums establish correspondence with the pinned independent
archive. They do not authenticate that archive as Mat Dickie's original
release. Original source notices remain unchanged. See the repository NOTICE.
