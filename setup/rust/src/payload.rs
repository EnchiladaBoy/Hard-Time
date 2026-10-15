use crate::{Cancellation, files, transaction};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{Cursor, Read, Seek, SeekFrom},
    path::Path,
};

pub const MAGIC: &[u8; 16] = b"HARDTIME-RUST-01";
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub game_sha256: String,
    pub files: BTreeMap<String, String>,
}

pub fn bundled(executable: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(executable)?;
    let size = file.metadata()?.len();
    ensure!(
        size >= 64,
        "Setup payload is missing. Use the packaged Hard-Time-Setup.exe."
    );
    file.seek(SeekFrom::End(-64))?;
    let mut footer = [0_u8; 64];
    file.read_exact(&mut footer)?;
    ensure!(
        &footer[..16] == MAGIC,
        "Setup payload is missing. Use the packaged Hard-Time-Setup.exe."
    );
    let offset = u64::from_le_bytes(footer[16..24].try_into()?);
    let length = u64::from_le_bytes(footer[24..32].try_into()?);
    ensure!(
        length > 0
            && length <= MAX_ARCHIVE
            && offset >= 64
            && offset.checked_add(length) == Some(size - 64),
        "Setup payload is damaged. Download setup again."
    );
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; length as usize];
    file.read_exact(&mut bytes)?;
    ensure!(
        Sha256::digest(&bytes)[..] == footer[32..],
        "Setup payload checksum failed. Download setup again."
    );
    Ok(bytes)
}

fn allowed(name: &str) -> bool {
    name == "manifest.json"
        || name
            .strip_prefix("runtime/")
            .is_some_and(|path| transaction::allowed(path, true))
        || [
            "tools/7z.exe",
            "tools/7z.dll",
            "software/opengl32.dll",
            "software/libgallium_wgl.dll",
            "software/software-renderer.txt",
        ]
        .contains(&name)
}

pub fn unpack(bytes: &[u8], root: &Path, cancel: &Cancellation) -> Result<Manifest> {
    cancel.check()?;
    files::private_path(root)?;
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).context("Setup's archive is damaged.")?;
    ensure!(archive.len() <= 5000, "Too many bundled files.");
    let mut names = HashSet::new();
    let mut total = 0_u64;
    // Check the complete entry set before extraction, including Windows aliases.
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name()?;
        files::entry_path(root, &name)?;
        ensure!(
            allowed(&name) && !entry.is_dir() && names.insert(name.to_ascii_lowercase()),
            "Unexpected or duplicate bundled file."
        );
        ensure!(
            entry
                .unix_mode()
                .is_none_or(|mode| mode & 0o170000 == 0 || mode & 0o170000 == 0o100000),
            "Links are not allowed in setup packages."
        );
        total = total
            .checked_add(entry.size())
            .context("Archive size overflow.")?;
        ensure!(
            entry.size() <= 512 * 1024 * 1024 && total <= 1024 * 1024 * 1024,
            "Setup archive is too large."
        );
        if name == "manifest.json" {
            ensure!(
                entry.size() <= 2 * 1024 * 1024,
                "Setup manifest is too large."
            );
        }
    }
    for index in 0..archive.len() {
        cancel.check()?;
        let entry = archive.by_index(index)?;
        let path = files::entry_path(root, &entry.name()?)?;
        files::private_path(&path)?;
        fs::create_dir_all(path.parent().unwrap())?;
        let mut output = File::create_new(&path)?;
        let expected_size = entry.size();
        // Bound actual decompression too, not just the archive's claimed size.
        let copied = std::io::copy(&mut entry.take(expected_size + 1), &mut output)?;
        ensure!(copied == expected_size, "Bundled file size mismatch.");
        output.sync_all()?;
    }
    let manifest: Manifest = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
    ensure!(
        manifest.schema == 1 && manifest.files.len() + 1 == names.len(),
        "Invalid setup manifest."
    );
    let mut declared = HashSet::new();
    for (name, expected) in &manifest.files {
        cancel.check()?;
        let path = files::entry_path(root, name)?;
        ensure!(
            name != "manifest.json"
                && allowed(name)
                && names.contains(&name.to_ascii_lowercase())
                && declared.insert(name.to_ascii_lowercase())
                && expected.len() == 64
                && files::hash(&path)? == *expected,
            "Setup's bundled files are damaged. Download setup again."
        );
    }
    for name in [
        "runtime/Hard Time.exe",
        "runtime/Play Hard Time.exe",
        "runtime/OpenAL32.dll",
        "runtime/NOTICE.txt",
        "runtime/Read Me.txt",
        "tools/7z.exe",
        "tools/7z.dll",
        "software/opengl32.dll",
        "software/libgallium_wgl.dll",
        "software/software-renderer.txt",
    ] {
        ensure!(
            manifest.files.contains_key(name),
            "Setup is missing {name}."
        );
    }
    ensure!(
        files::hash(&root.join("runtime/Hard Time.exe"))? == manifest.game_sha256,
        "The bundled game checksum does not match."
    );
    Ok(manifest)
}
