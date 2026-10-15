HARD TIME - RUST SETUP
=====================

Run the packaged Hard-Time-Setup.exe. Select the original installer or a
complete original game folder, or let setup download it from MDickie.
Click Install game, then Play Hard Time. The default destination is your
Local AppData/Programs/Hard Time folder. Administrator rights are unnecessary.

This installer and its Play launcher are native Rust programs. Players need
no Rust, .NET, Python, build tools or separate 7-Zip installation. Original
assets are downloaded/copied separately; they are never bundled with setup.
The installed game is the complete BASIC/Blitz3D compatibility build. The
Rust game runtime remains a separate environment preview.

The original installer is checksum-checked and extracted, never run. Downloads
use Windows HTTPS/certificate/proxy support, a size limit and the pinned hash.
The known original is:
  https://www.mdickie.com/downloads/new/hardtime_setup.exe
  SHA256 72d4f6a2e9c22a77f63b2f90730cb71bb742ecf9c1782000f9a5e4cb641c6ca1
Other versions/mods must be supplied as complete extracted folders.

Updating and recovery
---------------------
Close the game and run setup again with the same destination. Existing Data,
Photo Album and backups are preserved. Graphics can be changed in place.
Automatic graphics checks the system driver for OpenGL 3.3 on a new install
and retains the existing choice on updates. Compatibility graphics uses
app-local Mesa; it can be slow and never changes system drivers.

Setup recognizes installations and interrupted updates made by the previous
C# installer. It rejects overlapping/unrelated destinations and links or
junctions. Program changes use a durable rollback journal. Rerun setup after
an interrupted update. Cancellation is available until the final commit;
network operations use five-second timeouts and check cancellation between calls.

Shortcuts use the Rust Play Hard Time.exe launcher. It locks the installation,
recovers recognized interrupted save switches, backs up Data and records
stdout/stderr under logs. Use this launcher normally. Setup also refuses to
replace a directly running game or launcher.

Build and checks (maintainers only)
----------------------------------
Use Windows x64, the repository's pinned Rust MSVC toolchain and Visual Studio
C++ build tools/Windows SDK. The setup workspace is independent of the Bevy
game workspace. Its Cargo.lock pins dependencies. From the repository root:

  cargo fmt --manifest-path setup/rust/Cargo.toml --check
  cargo test --locked --manifest-path setup/rust/Cargo.toml
  cargo clippy --locked --manifest-path setup/rust/Cargo.toml --all-targets -- -D warnings
  python scripts/package-windows.py
  python tests/test_setup_windows.py

Packaging first needs the existing native game built by scripts/windows.py.
It builds both Rust programs, appends a checked ZIP payload to setup and
includes Rust/native dependency notices. The static CRT is selected by the
repository's .cargo/config.toml. Unpackaged cargo binaries have no payload.

Automation uses the previous setup CLI:
  Hard-Time-Setup.exe --install --download-original --destination C:/Games/HardTime
  Hard-Time-Setup.exe --install --source C:/original/hardtime_setup.exe --destination C:/Games/HardTime --renderer software --no-shortcuts --result C:/result.json
Renderer: auto, gpu or software. Exit 0 means success. --result is optional
and must be outside the original and installed game folders. With arguments,
errors go to stderr/the result JSON; no modal window blocks automation.

The preview is unsigned. Hard Time is copyright Mat Dickie 2007; see NOTICE.txt
for the original name, credits and noncommercial conditions. Runtime changes
and the alarm mute come from the existing native build, not from setup.
