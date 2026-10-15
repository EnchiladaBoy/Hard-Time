HARD TIME - NATIVE WINDOWS x64 AND ARM64 LINUX PORTS

An unofficial port of Mat Dickie's original Hard Time, built from an archived
BASIC source tree and a corrected Blitz3D NG runtime. The game runs as a native
Windows x64 or ARM64 Linux executable using SDL, OpenGL and OpenAL.

Status: experimental, with a playable Windows x64 build. Windows 11 testing
uses app-local Mesa llvmpipe on a machine with Microsoft Basic Display Adapter.
Menus, character creation, courtroom introduction, gameplay, movement,
photographs, save/reload and saving on window close have been exercised.
Hardware graphics, physical controllers and a full playthrough remain untested
on Windows. The Linux port retains its earlier Fedora 44 ARM64 validation.

WHAT IS HERE

  patches/       Game, engine and dependency corrections.
  scripts/       Pinned source preparation, native build and local installation.
  tests/         Small BASIC regressions and a test runner.
  tools/         Read-only analysis of the original game's Blitz3D module.
  provenance/    Upstream revisions, source hashes and historical validation.
  docs/          Compatibility notes and original engine license.

Game artwork, sounds, models, the original installer and your saves are supplied
locally. Downloaded source trees and build products live in the ignored .work/
directory. The repository does not contain the original complete game or any
player's saved progress.

WINDOWS

The installer creates dist/Hard-Time-Windows-x64 locally. Launch its
"Play Hard Time.cmd" file. Data backups and run logs are kept inside that
installation. It uses separate copies of the supplied original game's data.

To rebuild, install Python 3.11+, Git, CMake, 7-Zip, and Visual Studio 2022
Build Tools with Desktop development with C++ (including Windows SDK, CMake
tools/Ninja and DIA SDK). From the repository in PowerShell:

  python scripts/windows.py
  python scripts/install-windows.py --installer "$env:USERPROFILE\Downloads\hardtime_setup.exe" --software-renderer

The installer is extracted without executing it. Omit --software-renderer
for a separate installation using an OpenGL 3.3-capable hardware driver.
See scripts/README-windows.txt for prerequisites, paths and renderer selection.
Original 32-bit save serialization is retained in the new 64-bit executable.

LINUX

Build on ARM64 Linux with a C/C++ compiler, CMake, Ninja, Python 3, Git,
LLVM/LLD 19 development libraries, and the native development dependencies
listed in scripts/README.txt. Install Comfortaa and Liberation fonts. An X11
display (including XWayland) and OpenGL support are required to run the game.

  python3 scripts/prepare.py
  scripts/build.sh
  scripts/install.sh /path/to/extracted-original-game "$HOME/Games/Hard-Time-native"

The source revisions and all 18 original BASIC file hashes are pinned.
Preparation applies the corrections locally. See scripts/README.txt for exact
paths, prerequisites, overrides, and use of an existing build environment.
The first build downloads upstream sources and builds the compiler and engine.

Installation requires the extracted original game's Characters, Graphics,
Sound, Weapons, World and Data directories. It creates a separate local game
directory and preserves its existing saves when run again. Follow the launch
instructions printed by the installer. Save backups are made before launching.

VALIDATION

See tests/TESTS.txt for running the focused regressions. Tests use synthetic
files in temporary directories and do not use your live game saves. Graphics
tests need a working OpenGL display; passing them does not establish complete
game compatibility. Historical gameplay results and their limits are recorded
in provenance/validation.json. Windows build and test evidence is recorded
separately in provenance/windows-validation.json.

The historical Linux build showed about 23.6 displayed changes per second
in a 15-second busy-hall sample at 800x600 on a four-vCPU software-rendered VM.
That measurement is a lower bound from window-pixel sampling, not an internal
FPS counter or a promise for other hardware/scenes. The early near-30 result
predated rendering fixes and is not the final native performance result.

ORIGIN AND CREDITS

Original game: Mat Dickie, Copyright 2007. Its name, credits and original
conditions are retained. This is an unofficial compatibility project.

Archived game source:
  https://github.com/softbie/hardtime-rebirth
  7fdfd95a71cc5b29974928b9a1151c1f3d6d0768

Engine:
  https://github.com/blitz3d-ng/package
  d8d5d1be34dfac328e15d9e9b831d9572cde9d0f

All 233 named game functions correspond to symbols recovered from the supplied
executable. This establishes close correspondence, not an authenticated source
release, a byte-identical rebuild or proof that every behavior is unchanged.
See NOTICE.txt and provenance/game-source.json.
