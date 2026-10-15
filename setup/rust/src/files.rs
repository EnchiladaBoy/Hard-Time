use crate::Cancellation;
use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const MARKER: &str = ".hardtime-native-install.json";
pub const ASSETS: [&str; 6] = [
    "Characters",
    "Graphics",
    "Sound",
    "Weapons",
    "World",
    "Data",
];

pub fn absolute(path: &Path) -> Result<PathBuf> {
    ensure!(!path.as_os_str().is_empty(), "Choose a folder first.");
    let path = std::path::absolute(path)?;
    for part in path.components() {
        ensure!(
            !matches!(part, Component::ParentDir),
            "Choose a path without '..'."
        );
    }
    Ok(path)
}

pub fn within(path: &Path, parent: &Path) -> bool {
    let p = path
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase();
    let root = parent
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase();
    p == root || p.starts_with(&(root + "\\"))
}

/// Reject junctions, symbolic links and other reparse points, including dangling
/// links. Check every existing ancestor before creating or modifying entries.
pub fn private_path(path: &Path) -> Result<()> {
    for ancestor in absolute(path)?.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) => ensure!(
                meta.file_attributes() & 0x400 == 0,
                "Choose an ordinary folder, not a link or junction: {}",
                ancestor.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => {
                return Err(e).with_context(|| format!("Cannot inspect {}", ancestor.display()));
            }
        }
    }
    Ok(())
}

pub fn check_tree(path: &Path) -> Result<()> {
    private_path(path)?;
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            check_tree(&entry?.path())?;
        }
    }
    Ok(())
}

pub fn copy_tree(source: &Path, destination: &Path, cancel: &Cancellation) -> Result<()> {
    cancel.check()?;
    private_path(source)?;
    private_path(destination)?;
    ensure!(source.is_dir(), "Missing game folder: {}", source.display());
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        cancel.check()?;
        let entry = entry?;
        let path = entry.path();
        private_path(&path)?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&path, &target, cancel)?;
        } else {
            copy_new(&path, &target)?;
        }
    }
    Ok(())
}

pub fn copy_new(source: &Path, destination: &Path) -> Result<()> {
    private_path(source)?;
    private_path(destination)?;
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(())
}

pub fn remove_owned(path: &Path, root: &Path) -> Result<()> {
    let path = absolute(path)?;
    let root = absolute(root)?;
    ensure!(
        within(&path, &root) && path != root,
        "Cleanup path is outside its owned folder."
    );
    check_tree(&path)?;
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.is_file() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn durable(path: &Path, contents: &[u8]) -> Result<()> {
    private_path(path)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    Ok(())
}

pub fn hash(path: &Path) -> Result<String> {
    private_path(path)?;
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn lock(root: &Path) -> Result<File> {
    let path = root.join(".launch.lock");
    private_path(&path)?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(path)
        .context("Close Hard Time and any other setup window, then try again.")
}

pub fn ensure_game_closed(root: &Path) -> Result<()> {
    // Check both binaries: replacing a running Rust launcher would also fail.
    for name in ["Hard Time.exe", "Play Hard Time.exe"] {
        let path = root.join(name);
        private_path(&path)?;
        if path.exists() {
            OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(path)
                .context(
                    "Close Hard Time before updating, including any directly launched game.",
                )?;
        }
    }
    Ok(())
}

pub fn recover_data(root: &Path) -> Result<()> {
    let data = root.join("Data");
    let marker = root.join("Data.transaction");
    private_path(&data)?;
    private_path(&marker)?;
    ensure!(!data.is_file(), "A file is blocking the Data folder.");
    if data.is_dir() || !marker.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(marker)?;
    ensure!(
        text.lines().eq(["Hard Time save transaction v2", "Data"]),
        "The save recovery marker is unrecognized. Keep this folder intact so your saves can be recovered."
    );
    let previous = root.join("Data.previous");
    check_tree(&previous)?;
    ensure!(
        previous.is_dir(),
        "The previous save folder is missing. Setup will not replace your recovery data."
    );
    fs::rename(previous, data)?;
    Ok(())
}

/// Strict portable package names prevent traversal, NTFS streams, device names
/// and Windows trailing-dot/space aliases. Extraction never follows links.
pub fn entry_path(root: &Path, relative: &str) -> Result<PathBuf> {
    ensure!(
        !relative.is_empty() && relative.is_ascii() && !relative.contains(['\\', ':', '\0']),
        "Invalid package path: {relative}"
    );
    for part in relative.split('/') {
        ensure!(
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part.contains(['<', '>', '"', '|', '?', '*'])
                && !part.bytes().any(|b| b < 32),
            "Invalid package path: {relative}"
        );
        let stem = part.split('.').next().unwrap().to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            bail!("Invalid package device name: {relative}");
        }
    }
    Ok(root.join(relative))
}
