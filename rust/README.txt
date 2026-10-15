Hard Time: Rust migration preview 01
====================================

Hard Time is copyright Mat Dickie 2007. This is an unofficial,
noncommercial migration of the original game. See the repository/package
NOTICE.txt.

This preview starts a new Rust game runtime. It does not launch or link
Blitz3D. The existing Windows setup remains the way to play the full game.
The installer and launcher are now implemented separately in Rust; see
setup/rust/README.txt. They install the existing complete compatibility game.

Current scope
-------------
- A Rust/Bevy window and Direct3D 12 renderer on Windows x64.
- Explore nine original locations with a free-flying camera.
- Original 3DS static geometry, material groups and texture coordinates.
- Masked shared fence/shower textures and the first courtroom crowd frame.
- Display-synchronized presentation, independent of a fixed 30 Hz clock.
  Camera translation is interpolated between simulation steps.
- Read-only save-slot inspection. Binary readers preserve the original
  options, progress, character, item and modern-settings formats.
- Pause on focus loss, an FPS/tick counter and asynchronous screenshots.

This is an environment preview, not the complete playable Rust game.
There are no characters, combat, AI, collision, missions, dialogue, game
audio or save-writing yet. Character animation and dynamic world textures
also remain to port. Importing a save does not yet resume gameplay.
All audio is currently absent, including the alarm; the alarm must remain
silent when the other audio systems are ported.

Run the Windows preview
-----------------------
Extract the complete preview ZIP. No Rust toolchain or separate runtime
installation is required for the packaged executable. Use an existing
original/native game folder containing Characters, Graphics, Sound,
Weapons, World and Data. Original assets are not included in this package.

Drop that game folder onto Hard-Time-Rust.exe, or run:

  .\Hard-Time-Rust.exe --assets "C:\Games\Hard Time"

The preview also looks beside its executable, in the working directory,
in dist/Hard-Time-Windows-x64 from a repository checkout, and at
%LOCALAPPDATA%\Programs\Hard Time (the existing setup's default location).
The current setup installs the earlier full game, not this Rust preview.

Windows 10/11 x64 with Direct3D 12 is the target. A recent graphics driver
is recommended. --software selects the Windows software adapter for
diagnostics; performance will be lower. Hardware GPUs and high-refresh
monitors need separate validation. AutoVsync requests the display refresh
rate; it cannot guarantee that the hardware can render every frame.

Controls
--------
Enter              Explore the current location
Tab                Next location
WASD / arrows      Fly horizontally
Q / E              Down / up
Hold right mouse   Look around
Left Shift         Move faster
P                  Pause / resume (also resume after focus loss)
Esc                Open / close the menu
S in the menu      Inspect existing save slots
F11                Toggle the FPS/tick counter
F12                Save a PNG under %LOCALAPPDATA%\Hard Time Rust\Screenshots

The preview never writes to the source game folder or its saves.

Build and validate
------------------
Maintainers need the Rust MSVC toolchain and Visual Studio Build Tools
with Desktop development with C++ and the Windows SDK. rust-toolchain.toml
pins the Rust version; Cargo.lock pins dependencies. Python 3.11+ is needed
only for packaging.

From the repository root:

  cargo build --release --locked -p hardtime-rust
  cargo run --locked -p hardtime-rust -- --assets "C:\Games\Hard Time"
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test --locked -p hardtime-core
  cargo run --locked -p hardtime-core --example inspect -- "C:\Games\Hard Time"
  python scripts/package-rust.py
  python tests/test_rust_package.py

The core tests use synthetic data and need no game assets. The inspect
example checks every populated save slot byte for byte, then parses all
original 3DS files; it does not modify them. The GUI smoke test needs local
original assets and an interactive Windows desktop:

  .\target\release\hardtime-rust.exe --assets "C:\Games\Hard Time" --software --smoke-test "new-evidence-directory"

The output directory must not exist. The test exercises all nine locations,
30 movement ticks, the save-slot menu and twelve asynchronous captures,
then exits. Its
result.txt records capture events; inspect the PNGs to assess rendering.
Software FPS is not a measurement of hardware-GPU performance.

Code and migration order
-----------------------
rust/core: Standard-library-only binary formats, bounded asset/model
           readers and deterministic clock. No engine dependency.
rust/game: Bevy application, renderer, camera, menus and screenshot path.

The archived BASIC source is already available and pinned in provenance/.
A complete new Ghidra decompilation is not a prerequisite. The old build
remains a behavioral reference while these systems are moved in order:

1. Character hierarchy/animation, movement and world collision.
2. Combat, weapons and character statistics.
3. NPC AI, prison events, missions and dialogue.
4. Complete menus and audio (with the alarm still silent).
5. Save-writing parity and gameplay regression comparisons.
6. Rust installer and launcher: implemented in setup/rust, using the existing
   asset acquisition flow. Packaging the complete Rust game will follow parity.

Port gameplay rules into the independent core where practical. Preserve
30 Hz behavior and original save schemas; render interpolation belongs
in the presentation layer. Validate changes against the archived source
and reference executable before replacing the existing playable build.
