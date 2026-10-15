WINDOWS x64 BUILD AND LOCAL INSTALLATION

Target: native x64 Windows with OpenGL 3.3. Tested on Windows 11 Pro, build
26200, using the optional app-local Mesa renderer. Windows 10 and GPU drivers
are target configurations but have not been validated here. ARM64 Windows
and 32-bit Windows are outside this workflow.

Prerequisites
  Python 3.11 or newer, Git for Windows, CMake and 7-Zip.
  Visual Studio 2022 Build Tools with Desktop development with C++, Windows
  SDK, CMake tools/Ninja and DIA SDK. MFC and the legacy Blitz IDE are not used.
  Roughly 15 GB of build space and an internet connection for the first build.

Build from PowerShell in the repository:
  python scripts/windows.py --jobs 2

The script imports an x64 Visual Studio developer environment itself. It
uses vswhere to find the installed toolset and SDK instead of fixed versions.
Python, Git and CMake must be installed; Git, CMake, Ninja and 7-Zip are also
discovered in their usual Windows installation locations.

The build prepares the pinned engine and all 18 checksum-verified BASIC
sources, applies the shared corrections and then the Windows patches, builds
the compiler and runtime, and produces .work/windows/game/Hard Time.exe.
The executable is PE32+ AMD64, links the C/C++ runtime statically, embeds an
asInvoker/DPI-aware manifest, and uses app-local OpenAL Soft for audio.

LLVM/LLD 19.1.5, OpenAL Soft 1.25.2 and optional Mesa 26.2.4 archives are pinned
by URL and SHA256 in windows-toolchain.json. Downloads and extracted tools
are cached below .work/downloads and .work/toolchain. A checksum mismatch
stops the build. No Windows driver or DirectX installation is performed.

Install using the supplied original game:
  python scripts/install-windows.py --installer "$env:USERPROFILE\Downloads\hardtime_setup.exe" --software-renderer

Or use an already extracted original game:
  python scripts/install-windows.py --assets C:\Games\HardTimeOriginal --destination C:\Games\HardTimeNative

--installer extracts the NSIS archive with 7-Zip; it does not execute it.
Required original directories: Characters, Graphics, Sound, Weapons, World,
and Data. The original executable is not used to run the native game.

The default destination is dist/Hard-Time-Windows-x64. Installation copies
the original assets and save files only when absent. Reinstallation recovers
an interrupted Data-directory switch before checking for missing files. It updates
the native executable, launcher and runtime DLLs without replacing existing
Data or Photo Album contents. Unrelated directories, overlapping original
paths and updates while this launcher's game is running are refused.

Renderer choice
  Default: use the installed GPU's OpenGL 3.3 driver.
  --software-renderer: copy Mesa's x64 opengl32.dll and libgallium_wgl.dll
  beside the game and select llvmpipe. Suitable for Microsoft Basic Display
  Adapter and virtual machines; performance depends on the CPU.

Use a new destination when changing renderer mode so old DLLs cannot silently
override the new selection. With both copies closed, Data and Photo Album
can be copied between native installations to retain progress.

Play and saves
  Double-click Play Hard Time.cmd in the installed folder. It starts the
  game in its own working directory, backs up Data into backups/<timestamp>,
  prevents concurrent use of those saves, and writes diagnostics into logs.
  Do not put the installation in a protected directory such as Program Files.
  Esc saves active gameplay to the slot menu; closing the window also saves.
  Save-slot labels are persisted with progress instead of only at Quit.
  Grouped saves are written into Data.saving, flushed, then switched into
  place. Data.previous holds the preceding committed generation. A marker
  identifies owned recovery folders; unknown recovery folders are preserved
  and reported rather than overwritten. The launcher and installer recover
  a missing Data folder from Data.previous before making backups or updating.
  Recovery markers remain valid when the installation folder is moved.
  Atomic option writes retain .bak copies. Settings are size/range checked
  before loading; damaged copies are kept for diagnosis. NativeOptions.dat
  adds modern preferences without extending the legacy binary Options format.
  Resetting an occupied slot requires an explicit confirmation.
  F12 creates a BMP in Photo Album. The original manual is copied when present.
  Read Me.txt in the installation explains restoring a backup.

Display refresh and diagnostics
  Gameplay and scripted scenes use interpolated rendering with a fixed 30 Hz
  simulation. Menus and dialogue replay recorded UI drawing between ticks;
  scripted UI functions and gameplay RNG are only evaluated on logic frames.
  Presentation follows driver vsync. If the driver rejects vsync, a monotonic
  limiter uses the window's display refresh rate, falling back to 60 Hz when
  the display does not report a rate. This removes the gameplay render cap;
  actual FPS still depends on the scene and graphics driver. Loading feedback
  updates as assets become ready. Returning from a modal confirmation discards
  that modal's elapsed UI time; ordinary render load retains timer catch-up.
  F11 toggles the rendered-FPS/simulation-Hz counter during gameplay.
  Optional environment variables, set before launching:
    HARDTIME_SHOW_FPS=1       show the counter immediately
    HARDTIME_ORIGINAL_FPS=1   use the original blocking 30 Hz rendering
    HARDTIME_FPS_LOG=1        write frame-times.csv in the installation
  The CSV contains approximately two-second aggregates, including loading and
  pause, with update/render/UI/presentation milliseconds for profiling. It is
  overwritten when entering gameplay; copy it elsewhere before another run.
  Diagnostic settings do not change the save-file format.

Modern settings
  Options > Modern Settings offers desktop/native, 1080p, 1440p and 4K sizes,
  borderless desktop mode, HUD scaling, camera sensitivity/distance, focus-loss
  pause and independent music/effects/ambience gains. Display changes keep the
  GL context and loaded assets alive and have a 15-second confirmation timeout.
  Options > Redefine Keys includes movement, actions, pause and a WASD preset.
  Duplicate bindings swap rather than assigning one key to two actions.
  The SDL input driver discovers hotplug events and clears disconnected input.
  The alarm's zero base gain is preserved across all category-volume changes.
  Dedicated Audio Settings and Cheats entries are available in Options and
  pause. Master/mute multiply the existing category gains. NativeExtras.dat is
  a checksum/range-checked extension, leaving NativeOptions.dat unchanged.
  Nonlethal limb breaker, invincibility and super knockback default off. Limb
  loss uses the existing character save format. Detached effects are limited
  to 24 entities and three seconds, updated only on simulation ticks.
  Dialogue/courtroom bars use the whole drawable, then restore the UI transform.

Renderer maintenance
  Mesh buffers/arrays, scene resources, light representations, cached font
  textures and text geometry are released while the owning GL context exists.
  Index buffers upload only when triangle data changes. Text geometry uses a
  bounded cache (256 strings), invalidated when the font atlas changes.
  Recorded UI commands are invalidated when a referenced resource is freed.

Regression checks (after building):
  python tests/run.py --all --post-flip --software-renderer
  python -m unittest discover -s tests -p test_windows_install.py -v
  python tests/test_controller_windows.py
  python tests/test_game_features_windows.py --assets dist/Hard-Time-Windows-x64

The regression runner creates isolated temporary files and uses silent OpenAL.
It does not modify live saves or prove audible output. See tests/TESTS.txt.

Advanced paths
  HARDTIME_WORK_DIR changes the build/prepared-source directory.
  A changed patch recipe requires a fresh work directory; existing edits are
  never discarded automatically. For example:
    $env:HARDTIME_WORK_DIR="$PWD\.work\windows-next"
    python scripts/windows.py
  HARDTIME_ENGINE_REFERENCE can reference a local engine Git clone to avoid
  downloading objects. HARDTIME_GAME_SOURCE_REFERENCE can reference the
  original, unpatched, hash-verified .bb files. No source hashes are bypassed.

The Linux shell workflow remains available; see scripts/README.txt.
