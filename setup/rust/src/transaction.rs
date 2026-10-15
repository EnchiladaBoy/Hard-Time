use crate::files::{self, ASSETS, MARKER};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};

// Compatible with the existing C# setup's recovery format.
pub const DIRECTORY: &str = ".hardtime-setup";
pub const OWNER: &str = "Hard Time setup transaction v1\n";
pub const MANAGED: [&str; 12] = [
    "Hard Time.exe",
    "Play Hard Time.exe",
    "OpenAL32.dll",
    "opengl32.dll",
    "libgallium_wgl.dll",
    "software-renderer.txt",
    "Read Me.txt",
    "NOTICE.txt",
    "play.ps1",
    "Play Hard Time.cmd",
    MARKER,
    "Rust Setup.txt",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
    pub path: String,
    pub had_original: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Journal {
    pub schema: u32,
    pub operations: Vec<Operation>,
}

impl Default for Journal {
    fn default() -> Self {
        Self {
            schema: 1,
            operations: Vec::new(),
        }
    }
}

pub fn allowed(relative: &str, had_original: bool) -> bool {
    MANAGED.contains(&relative)
        || (relative.starts_with("licenses/") && relative.split('/').count() == 2)
        || (!had_original
            && (ASSETS.contains(&relative)
                || [
                    "Photo Album",
                    "Instruction Manual.txt",
                    "Survival Guide.txt",
                ]
                .contains(&relative)))
}

pub fn validate(root: &Path, journal: &Journal) -> Result<()> {
    ensure!(
        journal.schema == 1 && journal.operations.len() <= 5000,
        "Invalid setup recovery journal."
    );
    let mut seen = HashSet::new();
    for item in &journal.operations {
        let path = files::entry_path(root, &item.path)?;
        ensure!(
            allowed(&item.path, item.had_original) && seen.insert(item.path.to_ascii_lowercase()),
            "Unexpected path in setup recovery journal."
        );
        files::private_path(&path)?;
    }
    Ok(())
}

pub fn recover(root: &Path) -> Result<()> {
    let transaction = root.join(DIRECTORY);
    files::check_tree(&transaction)?;
    if !transaction.exists() {
        return Ok(());
    }
    ensure!(
        transaction.is_dir(),
        "The setup recovery path is not a folder."
    );
    if fs::read_dir(&transaction)?.next().is_none() {
        fs::remove_dir(transaction)?;
        return Ok(());
    }
    ensure!(
        fs::read(transaction.join("owner"))? == OWNER.as_bytes(),
        "The setup recovery folder is unrecognized. Keep it intact and choose another folder."
    );
    let journal_path = transaction.join("journal.json");
    if journal_path.exists() && !transaction.join("committed").exists() {
        ensure!(
            fs::metadata(&journal_path)?.len() <= 2 * 1024 * 1024,
            "Setup journal is too large."
        );
        let journal: Journal = serde_json::from_slice(&fs::read(journal_path)?)?;
        validate(root, &journal)?;
        // Validate every backup before making recovery changes.
        for item in &journal.operations {
            if item.had_original {
                ensure!(
                    !transaction.join("old").join(&item.path).is_dir()
                        && !root.join(&item.path).is_dir(),
                    "A folder is blocking setup recovery."
                );
            }
        }
        for item in journal.operations.iter().rev() {
            let target = root.join(&item.path);
            let backup = transaction.join("old").join(&item.path);
            let staged = transaction.join("new").join(&item.path);
            if item.had_original && backup.is_file() {
                if target.exists() {
                    fs::remove_file(&target)?;
                }
                move_entry(&backup, &target)?;
            } else if !item.had_original && !staged.exists() {
                files::remove_owned(&target, root)?;
            }
        }
    } else if !journal_path.exists() {
        ensure!(
            !transaction.join("old").exists(),
            "Setup recovery data has no journal. Keep this folder intact."
        );
    }
    files::remove_owned(&transaction, root)
}

fn move_entry(source: &Path, target: &Path) -> Result<()> {
    files::private_path(source)?;
    files::private_path(target)?;
    ensure!(
        !target.exists(),
        "A file is blocking setup: {}",
        target.display()
    );
    fs::create_dir_all(target.parent().unwrap())?;
    fs::rename(source, target)?;
    Ok(())
}

pub fn begin(root: &Path) -> Result<Journal> {
    let transaction = root.join(DIRECTORY);
    files::private_path(&transaction)?;
    fs::create_dir(&transaction)?;
    files::durable(&transaction.join("owner"), OWNER.as_bytes())?;
    Ok(Journal::default())
}

pub fn stage_file(
    source: Option<&Path>,
    relative: &str,
    root: &Path,
    journal: &mut Journal,
) -> Result<()> {
    let target = files::entry_path(root, relative)?;
    files::private_path(&target)?;
    ensure!(
        !target.is_dir(),
        "A folder is blocking this game file: {}",
        target.display()
    );
    let had_original = target.is_file();
    ensure!(
        allowed(relative, had_original),
        "Unknown bundled game file: {relative}"
    );
    let staged = root.join(DIRECTORY).join("new").join(relative);
    fs::create_dir_all(staged.parent().unwrap())?;
    if let Some(source) = source {
        files::copy_new(source, &staged)?;
    }
    journal.operations.push(Operation {
        path: relative.to_owned(),
        had_original,
    });
    Ok(())
}

/// The journal is durable before the first rename. The committed marker is
/// durable before cleanup. Recovery is idempotent at every rename boundary.
pub fn commit(
    root: &Path,
    journal: &Journal,
    mut after_move: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    validate(root, journal)?;
    let transaction = root.join(DIRECTORY);
    files::durable(
        &transaction.join("journal.tmp"),
        &serde_json::to_vec(journal)?,
    )?;
    fs::rename(
        transaction.join("journal.tmp"),
        transaction.join("journal.json"),
    )?;
    let mut count = 0;
    for item in &journal.operations {
        let target = root.join(&item.path);
        let staged = transaction.join("new").join(&item.path);
        if item.had_original {
            move_entry(&target, &transaction.join("old").join(&item.path))?;
            count += 1;
            after_move(count)?;
        }
        if staged.exists() {
            move_entry(&staged, &target)?;
            count += 1;
            after_move(count)?;
        }
    }
    files::durable(&transaction.join("committed"), b"complete\n")
}
