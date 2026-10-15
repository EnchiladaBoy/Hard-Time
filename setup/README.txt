HARD TIME STANDALONE SETUP

Player flow
  Run Hard-Time-Setup.exe on Windows 10/11 x64 and click Install game.
  If Downloads/hardtime_setup.exe exists, setup selects it automatically.
  Otherwise automatic download from MDickie's official HTTPS URL is selected.
  An extracted original game folder can also be selected. Install to a separate
  writable folder; the default is %LOCALAPPDATA%/Programs/Hard Time.
  Setup's Play button and the shortcuts use Play Hard Time.exe.

No player build tools
  Setup and the launcher are C# programs targeting the .NET Framework already
  included with Windows 10/11. The game itself remains the native x64 build.
  The single EXE embeds the game, OpenAL, Mesa, 7-Zip and dependency notices.
  No original assets, original executable or player data are embedded.
  Local source selection works offline. Automatic download needs a connection.
  The original NSIS installer is checksum-checked and extracted, never executed.
  Known original SHA256:
  72d4f6a2e9c22a77f63b2f90730cb71bb742ecf9c1782000f9a5e4cb641c6ca1
  https://www.mdickie.com/downloads/new/hardtime_setup.exe
  Other original revisions/mods must be supplied as a complete extracted folder.

Safety and recovery
  Setup refuses overlapping source/destination folders and unrelated nonempty
  destinations. Copies and writable paths reject links/junctions. A shared
  installation lock excludes the launcher and updater. Close games launched
  directly from Hard Time.exe as well; always use Play Hard Time.exe normally.
  Assets and Data are only seeded when their folders are missing. Existing
  Data, Photo Album and backups are preserved. A recognized interrupted save
  switch is recovered before fresh seed data is considered.
  Program replacements are staged with a durable rollback journal. Ordinary
  failures roll back. Rerunning setup recovers interrupted commits; a completed
  commit marker protects a successful update during interrupted cleanup.
  Cancellation is available before the short commit phase.
  Automatic graphics uses an isolated OpenGL probe on a new install and keeps
  the existing renderer on an update. Explicit renderer changes remove stale
  app-local Mesa files transactionally. GPU/driver behavior beyond this probe
  still needs hardware testing. The fallback does not change system drivers.
  The launcher holds the installation lock, backs up Data, records stdout and
  stderr, and displays a useful error if the game exits unsuccessfully.

Build (maintainers only, after scripts/windows.py)
  python scripts/package-windows.py
  python tests/test_setup_windows.py
  python tests/test_setup_windows.py --original-installer C:/path/hardtime_setup.exe

  Outputs: dist/Hard-Time-Setup.exe, .sha256 and .manifest.json
  The Windows Framework64 csc.exe compiler is used; no NuGet/.NET SDK restore.
  Python and 7-Zip are packaging dependencies only. The bundled 7-Zip version
  is downloaded and SHA256-pinned in scripts/windows-toolchain.json.
  --binary and --output select an alternate compiled game and destination.
  Rebuild setup after changing the game binary, launcher, notices or player guide.

GUI/download smoke test (interactive Windows session)
  powershell -NoProfile -ExecutionPolicy Bypass -File tests/test_setup_gui.ps1 -DownloadOriginal
  Uses an isolated installation, captures the real setup controls, clicks Install
  and Play, and leaves that disposable game open for visual checking. Close it
  normally afterwards. It never creates personal Desktop/Start menu shortcuts.

CLI for automation
  Hard-Time-Setup.exe --install --download-original --destination C:/Games/HardTime
  Hard-Time-Setup.exe --install --source C:/path/hardtime_setup.exe --destination C:/Games/HardTime --renderer software --no-shortcuts --result C:/path/result.json
  Renderer: auto (default), gpu, software. Exit code 0 means successful install;
  --result writes success details or an error. Without arguments the GUI opens.

GitHub packaging
  .github/workflows/windows-setup.yml builds from pinned source, runs the setup
  suite using synthetic assets, and uploads the EXE/checksum/manifest artifact.
  Pushing a new setup-v* tag additionally publishes a preview release after the
  checks pass. Original game assets are never uploaded. The original-download
  and actual-game GUI checks are local validation, not the hosted CI fixture test.
  Releases are unsigned. See NOTICE.txt for the original game's conditions and
  licenses/ for the runtime components' notices and source links.
