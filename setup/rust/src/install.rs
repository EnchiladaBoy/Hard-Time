use crate::{
    Cancellation,
    files::{self, ASSETS, MARKER},
    payload, platform,
    transaction::{self, Operation},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::{Component, Path, PathBuf, Prefix},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub const ORIGINAL_HASH: &str = "72d4f6a2e9c22a77f63b2f90730cb71bb742ecf9c1782000f9a5e4cb641c6ca1";
pub const ORIGINAL_URL: &str = "https://www.mdickie.com/downloads/new/hardtime_setup.exe";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Renderer {
    Auto,
    Gpu,
    Software,
}

impl std::str::FromStr for Renderer {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "auto" => Ok(Self::Auto),
            "gpu" => Ok(Self::Gpu),
            "software" => Ok(Self::Software),
            _ => bail!("Renderer must be auto, gpu or software."),
        }
    }
}

#[derive(Clone)]
pub struct Options {
    pub source: Option<PathBuf>,
    pub destination: PathBuf,
    pub renderer: Renderer,
    pub shortcuts: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Marker {
    pub platform: String,
    pub executable_sha256: String,
    pub software_renderer: bool,
    #[serde(default)]
    pub installer: String,
}

#[derive(Debug, Serialize)]
pub struct Installed {
    pub destination: PathBuf,
    pub renderer: String,
    pub warning: Option<String>,
}

fn read_marker(root: &Path) -> Result<Marker> {
    let path = root.join(MARKER);
    files::private_path(&path)?;
    ensure!(
        fs::metadata(&path)?.len() <= 65536,
        "Invalid installation marker."
    );
    let marker: Marker = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        marker.platform == "windows-x64"
            && marker.executable_sha256.len() == 64
            && marker
                .executable_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit()),
        "This folder is not a recognized native Hard Time installation."
    );
    Ok(marker)
}

pub fn validate_destination(destination: &Path, source: &Path) -> Result<()> {
    let first = destination.components().next();
    ensure!(
        matches!(first, Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_)))
            && destination.parent().is_some()
            && destination.as_os_str().len() <= 150,
        "Choose a short, local installation path, such as the suggested folder."
    );
    files::private_path(destination)?;
    files::private_path(&destination.join(MARKER))?;
    ensure!(
        !files::within(destination, source) && !files::within(source, destination),
        "Choose a separate destination so the original game stays intact."
    );
    ensure!(
        !destination.is_file(),
        "The destination is a file. Choose a folder."
    );
    if destination.join(MARKER).is_file() {
        read_marker(destination)?;
    } else if destination.is_dir() {
        let mut unrelated = false;
        for entry in fs::read_dir(destination)? {
            let name = entry?.file_name();
            if name != ".launch.lock" && name != transaction::DIRECTORY {
                unrelated = true;
            }
        }
        if unrelated {
            let owner = destination.join(transaction::DIRECTORY).join("owner");
            files::private_path(&owner)?;
            ensure!(
                fs::read(&owner).is_ok_and(|b| b == transaction::OWNER.as_bytes()),
                "That folder already contains other files. Choose an empty folder or an existing native Hard Time installation."
            );
        }
    }
    Ok(())
}

pub fn validate_assets(source: &Path) -> Result<()> {
    files::private_path(source)?;
    for name in ASSETS {
        let path = source.join(name);
        files::check_tree(&path)?;
        ensure!(
            path.is_dir() && fs::read_dir(&path)?.next().is_some(),
            "The original game folder is missing {name}. Select the original installer or a complete game folder."
        );
    }
    for name in [
        "Graphics/Logo01.PNG",
        "Data/Options.dat",
        "Data/Structure.dat",
    ] {
        ensure!(
            source.join(name).is_file(),
            "The original game is incomplete: {name} is missing."
        );
    }
    Ok(())
}

fn wait_child(command: &mut Command, cancel: &Cancellation, timeout: Duration) -> Result<bool> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.success());
        }
        if cancel.check().is_err() || start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            cancel.check()?;
            bail!("The operation timed out. Try selecting an extracted original game folder.");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn software_renderer(
    choice: Renderer,
    root: &Path,
    executable: &Path,
    cancel: &Cancellation,
) -> Result<bool> {
    match choice {
        Renderer::Gpu => Ok(false),
        Renderer::Software => Ok(true),
        Renderer::Auto if root.join(MARKER).is_file() => Ok(read_marker(root)?.software_renderer),
        Renderer::Auto => {
            let result = wait_child(
                Command::new(executable).arg("--probe-graphics"),
                cancel,
                Duration::from_secs(10),
            );
            cancel.check()?;
            Ok(!result.unwrap_or(false))
        }
    }
}

pub fn install(
    options: Options,
    executable: &Path,
    cancel: &Cancellation,
    mut progress: impl FnMut(u8, &str),
) -> Result<Installed> {
    let destination = files::absolute(&options.destination)?;
    let temp = std::env::temp_dir();
    files::private_path(&temp)?;
    let workspace = tempfile::Builder::new()
        .prefix("HardTime-Rust-Setup-")
        .tempdir_in(temp)?;
    let input = match &options.source {
        Some(path) => files::absolute(path)?,
        None => workspace.path().join("hardtime_setup.exe"),
    };
    validate_destination(&destination, &input)?;
    files::private_path(&input)?;
    if options.source.is_some() {
        ensure!(
            input.exists(),
            "Choose the original installer or game folder."
        );
    }
    cancel.check()?;
    progress(5, "Checking the bundled game and support files...");
    let bytes = payload::bundled(executable)?;
    let manifest = payload::unpack(&bytes, workspace.path(), cancel)?;
    drop(bytes);
    if options.source.is_none() {
        progress(10, "Downloading the original game from MDickie...");
        let downloaded = platform::download_original(&input, cancel, &mut progress);
        cancel.check()?;
        downloaded.context(
            "The original download failed. Check your connection or select a local original copy.",
        )?;
    }
    let source = if input.is_file() {
        ensure!(
            files::hash(&input)? == ORIGINAL_HASH,
            "This installer differs from the tested Hard Time (2007) installer. Select a complete extracted game folder instead."
        );
        progress(25, "Extracting the original game...");
        let source = workspace.path().join("original");
        fs::create_dir(&source)?;
        let mut command = Command::new(workspace.path().join("tools/7z.exe"));
        command
            .arg("x")
            .arg(&input)
            .arg(format!("-o{}", source.display()))
            .args(["-y", "-bso0", "-bse0", "-bsp0"])
            .current_dir(workspace.path());
        ensure!(
            wait_child(&mut command, cancel, Duration::from_secs(180))?,
            "The original installer could not be extracted. Select an extracted game folder instead."
        );
        source
    } else {
        input.clone()
    };
    validate_assets(&source)?;
    cancel.check()?;
    fs::create_dir_all(&destination)?;
    let lock = files::lock(&destination)?;
    files::ensure_game_closed(&destination)?;
    transaction::recover(&destination)?;
    validate_destination(&destination, &input)?;
    files::recover_data(&destination)?;
    progress(40, "Checking graphics support...");
    let software = software_renderer(options.renderer, &destination, executable, cancel)?;
    let mut journal = transaction::begin(&destination)?;
    let stage = (|| -> Result<()> {
        for name in manifest
            .files
            .keys()
            .filter_map(|name| name.strip_prefix("runtime/"))
        {
            cancel.check()?;
            transaction::stage_file(
                Some(&workspace.path().join("runtime").join(name)),
                name,
                &destination,
                &mut journal,
            )?;
        }
        for name in [
            "opengl32.dll",
            "libgallium_wgl.dll",
            "software-renderer.txt",
        ] {
            if software || destination.join(name).exists() {
                let file = workspace.path().join("software").join(name);
                transaction::stage_file(
                    software.then_some(file.as_path()),
                    name,
                    &destination,
                    &mut journal,
                )?;
            }
        }
        progress(
            50,
            "Copying game files. Existing saves and photos are kept...",
        );
        for name in ASSETS.into_iter().chain(["Photo Album"]) {
            let target = destination.join(name);
            files::private_path(&target)?;
            ensure!(
                !target.is_file(),
                "A file is blocking the game folder: {}",
                target.display()
            );
            if target.is_dir() {
                continue;
            }
            let staged = destination
                .join(transaction::DIRECTORY)
                .join("new")
                .join(name);
            if name == "Photo Album" {
                fs::create_dir_all(staged)?;
            } else {
                files::copy_tree(&source.join(name), &staged, cancel)?;
            }
            journal.operations.push(Operation {
                path: name.to_owned(),
                had_original: false,
            });
        }
        for name in ["Instruction Manual.txt", "Survival Guide.txt"] {
            if source.join(name).is_file() && !destination.join(name).exists() {
                transaction::stage_file(
                    Some(&source.join(name)),
                    name,
                    &destination,
                    &mut journal,
                )?;
            }
        }
        let marker = Marker {
            platform: "windows-x64".into(),
            software_renderer: software,
            executable_sha256: manifest.game_sha256,
            installer: "rust-0.1.0".into(),
        };
        let marker_path = workspace.path().join("install-marker.json");
        files::durable(&marker_path, &serde_json::to_vec_pretty(&marker)?)?;
        transaction::stage_file(Some(&marker_path), MARKER, &destination, &mut journal)?;
        cancel.check()?;
        progress(90, "Finishing installation...");
        // No cancellation after commit starts. The journal makes this restartable.
        transaction::commit(&destination, &journal, |_| Ok(()))
    })();
    if let Err(error) = stage {
        if let Err(recovery) = transaction::recover(&destination) {
            bail!(
                "{error:#}\nRecovery also needs attention: {recovery:#}. Run setup again before playing."
            );
        }
        return Err(error);
    }
    let mut warnings = Vec::new();
    if let Err(error) = transaction::recover(&destination) {
        warnings.push(format!(
            "The game is installed. Setup will finish temporary-file cleanup next time: {error}"
        ));
    }
    if options.shortcuts && platform::create_shortcuts(&destination, None).is_err() {
        warnings.push("The game is installed. A shortcut could not be created; open Play Hard Time.exe in the installation folder.".into());
    }
    drop(lock);
    progress(100, "Ready to play.");
    Ok(Installed {
        destination,
        renderer: if software { "software" } else { "gpu" }.into(),
        warning: (!warnings.is_empty()).then(|| warnings.join("\n")),
    })
}

pub fn launch(root: &Path) -> Result<i32> {
    let root = files::absolute(root)?;
    files::private_path(&root)?;
    let _lock = files::lock(&root)?;
    let recovery = root.join(transaction::DIRECTORY);
    files::check_tree(&recovery)?;
    ensure!(
        !recovery.exists() || recovery.join("committed").is_file(),
        "Setup was interrupted. Run setup again to finish recovery before playing."
    );
    files::recover_data(&root)?;
    let backups = root.join("backups");
    let logs = root.join("logs");
    files::private_path(&backups)?;
    files::private_path(&logs)?;
    files::private_path(&root.join("Hard Time.exe"))?;
    fs::create_dir_all(&backups)?;
    fs::create_dir_all(&logs)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let prefix = format!("before-launch-{stamp}-");
    let backup = tempfile::Builder::new()
        .prefix(&prefix)
        .tempdir_in(&backups)?;
    files::copy_tree(&root.join("Data"), backup.path(), &Cancellation::default())?;
    let backup_path = backup.keep();
    let name = backup_path.file_name().unwrap().to_string_lossy();
    let output = logs.join(format!("{name}.log"));
    let errors = logs.join(format!("{name}.errors.log"));
    let stdout = fs::File::create_new(&output)?;
    let stderr = fs::File::create_new(&errors)?;
    let mut command = Command::new(root.join("Hard Time.exe"));
    command
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .creation_flags(0x08000000);
    if root.join("software-renderer.txt").exists() {
        command.env("GALLIUM_DRIVER", "llvmpipe");
    }
    let status = command
        .status()
        .context("Hard Time could not start. Run setup again to repair the program files.")?;
    let latest = logs.join("latest.txt");
    files::private_path(&latest)?;
    // Rename a new file instead of opening a potentially hard-linked old log.
    let mut note = tempfile::NamedTempFile::new_in(&logs)?;
    use std::io::Write;
    writeln!(
        note,
        "{}\n{}\nExit status: {}",
        output.display(),
        errors.display(),
        status
    )?;
    note.as_file().sync_all()?;
    note.persist(&latest)?;
    ensure!(
        status.success(),
        "Hard Time exited unexpectedly. Diagnostics are in:\n{}\n\nIf the game did not open, rerun setup and choose Compatibility graphics. Your saves will be kept.",
        logs.display()
    );
    Ok(status.code().unwrap_or(1))
}
