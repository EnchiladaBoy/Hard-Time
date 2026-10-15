//! Recovery and preservation tests use disposable directories, never player data.
use anyhow::{Result, bail};
use hardtime_setup::{
    Cancellation, files, install, payload, platform,
    transaction::{self, Journal, Operation},
};
use std::{
    fs,
    io::{Cursor, Write},
    os::windows::fs::OpenOptionsExt,
    path::Path,
};

fn put(root: &Path, name: &str, value: &[u8]) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, value).unwrap();
}

fn prepare(root: &Path) -> Journal {
    put(root, "Hard Time.exe", b"old game");
    put(root, "opengl32.dll", b"old renderer");
    let mut journal = transaction::begin(root).unwrap();
    let source = root.join("input");
    fs::write(&source, b"new game").unwrap();
    transaction::stage_file(Some(&source), "Hard Time.exe", root, &mut journal).unwrap();
    transaction::stage_file(None, "opengl32.dll", root, &mut journal).unwrap();
    put(root, ".hardtime-setup/new/Data/fresh.dat", b"new data");
    journal.operations.push(Operation {
        path: "Data".into(),
        had_original: false,
    });
    transaction::stage_file(Some(&source), "Play Hard Time.exe", root, &mut journal).unwrap();
    fs::remove_file(source).unwrap();
    journal
}

#[test]
fn rollback_at_every_rename_boundary_is_idempotent() {
    for boundary in 1..=5 {
        let scratch = tempfile::tempdir().unwrap();
        let root = scratch.path();
        let journal = prepare(root);
        assert!(
            transaction::commit(root, &journal, |moved| {
                if moved == boundary {
                    bail!("simulated interruption");
                }
                Ok(())
            })
            .is_err()
        );
        install::validate_destination(root, &root.with_file_name("original")).unwrap();
        transaction::recover(root).unwrap();
        transaction::recover(root).unwrap();
        assert_eq!(fs::read(root.join("Hard Time.exe")).unwrap(), b"old game");
        assert_eq!(
            fs::read(root.join("opengl32.dll")).unwrap(),
            b"old renderer"
        );
        assert!(!root.join("Data").exists());
        assert!(!root.join("Play Hard Time.exe").exists());
    }
}

#[test]
fn committed_update_survives_interrupted_cleanup() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    let journal = prepare(root);
    transaction::commit(root, &journal, |_| Ok(())).unwrap();
    transaction::recover(root).unwrap();
    transaction::recover(root).unwrap();
    assert_eq!(fs::read(root.join("Hard Time.exe")).unwrap(), b"new game");
    assert!(!root.join("opengl32.dll").exists());
    assert_eq!(fs::read(root.join("Data/fresh.dat")).unwrap(), b"new data");
}

#[test]
fn cancelled_staging_keeps_existing_saves() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    put(root, "Data/player.dat", b"my save");
    transaction::begin(root).unwrap();
    put(root, ".hardtime-setup/new/Hard Time.exe", b"new program");
    transaction::recover(root).unwrap();
    assert_eq!(fs::read(root.join("Data/player.dat")).unwrap(), b"my save");
}

#[test]
fn missing_journal_preserves_recovery_data() {
    let scratch = tempfile::tempdir().unwrap();
    transaction::begin(scratch.path()).unwrap();
    put(
        scratch.path(),
        ".hardtime-setup/old/Hard Time.exe",
        b"precious old game",
    );
    assert!(transaction::recover(scratch.path()).is_err());
    assert!(
        scratch
            .path()
            .join(".hardtime-setup/old/Hard Time.exe")
            .is_file()
    );
}

#[test]
fn journals_cannot_replace_saves_or_repeat_paths() {
    for operations in [
        vec![("Data", true)],
        vec![("../important", true)],
        vec![("Photo Album", true)],
        vec![("Hard Time.exe", true), ("Hard Time.exe", true)],
        vec![("licenses/a.txt", false), ("licenses/A.TXT", false)],
    ] {
        let scratch = tempfile::tempdir().unwrap();
        put(scratch.path(), "Data/player.dat", b"player");
        transaction::begin(scratch.path()).unwrap();
        let journal = Journal {
            schema: 1,
            operations: operations
                .into_iter()
                .map(|(path, had_original)| Operation {
                    path: path.into(),
                    had_original,
                })
                .collect(),
        };
        put(
            scratch.path(),
            ".hardtime-setup/journal.json",
            &serde_json::to_vec(&journal).unwrap(),
        );
        assert!(transaction::recover(scratch.path()).is_err());
        assert_eq!(
            fs::read(scratch.path().join("Data/player.dat")).unwrap(),
            b"player"
        );
    }
}

#[test]
fn competing_launchers_and_setup_cannot_acquire_the_lock() {
    let scratch = tempfile::tempdir().unwrap();
    let held = files::lock(scratch.path()).unwrap();
    assert!(files::lock(scratch.path()).is_err());
    drop(held);
    files::lock(scratch.path()).unwrap();
}

#[test]
fn directly_opened_game_and_launcher_exclude_updates() {
    for name in ["Hard Time.exe", "Play Hard Time.exe"] {
        let scratch = tempfile::tempdir().unwrap();
        put(scratch.path(), name, b"program");
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(scratch.path().join(name))
            .unwrap();
        assert!(files::ensure_game_closed(scratch.path()).is_err());
        drop(held);
        files::ensure_game_closed(scratch.path()).unwrap();
    }
}

#[test]
fn interrupted_save_switch_recovers_byte_exact_data() {
    for marker in [
        "Hard Time save transaction v2\nData\n",
        "Hard Time save transaction v2\r\nData\r\n",
    ] {
        let scratch = tempfile::tempdir().unwrap();
        put(scratch.path(), "Data.previous/player.dat", b"\0\xffplayer");
        put(scratch.path(), "Data.transaction", marker.as_bytes());
        files::recover_data(scratch.path()).unwrap();
        files::recover_data(scratch.path()).unwrap();
        assert_eq!(
            fs::read(scratch.path().join("Data/player.dat")).unwrap(),
            b"\0\xffplayer"
        );
    }
}

#[test]
fn invalid_save_markers_and_missing_previous_data_are_preserved() {
    for marker in ["unknown\nData\n", "Hard Time save transaction v2\nData\n"] {
        let scratch = tempfile::tempdir().unwrap();
        put(scratch.path(), "Data.transaction", marker.as_bytes());
        assert!(files::recover_data(scratch.path()).is_err());
        assert!(!scratch.path().join("Data").exists());
        assert_eq!(
            fs::read(scratch.path().join("Data.transaction")).unwrap(),
            marker.as_bytes()
        );
    }
}

#[test]
fn overlapping_and_unrelated_destinations_are_refused() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    for (destination, source) in [
        (root.to_owned(), root.to_owned()),
        (root.join("native"), root.to_owned()),
        (root.to_owned(), root.join("original")),
    ] {
        assert!(install::validate_destination(&destination, &source).is_err());
    }
    put(root, "important.txt", b"keep");
    assert!(install::validate_destination(root, &root.with_file_name("original")).is_err());
    assert_eq!(fs::read(root.join("important.txt")).unwrap(), b"keep");
}

#[test]
fn old_csharp_install_markers_are_accepted() {
    let scratch = tempfile::tempdir().unwrap();
    put(
        scratch.path(),
        files::MARKER,
        serde_json::json!({"platform": "windows-x64",
        "software_renderer": true, "executable_sha256": "a".repeat(64)})
        .to_string()
        .as_bytes(),
    );
    install::validate_destination(scratch.path(), &scratch.path().with_file_name("original"))
        .unwrap();
}

#[test]
fn copies_never_overwrite_existing_files_or_follow_hardlinks() {
    let scratch = tempfile::tempdir().unwrap();
    put(scratch.path(), "original", b"new");
    put(scratch.path(), "important", b"keep");
    fs::hard_link(
        scratch.path().join("important"),
        scratch.path().join("target"),
    )
    .unwrap();
    assert!(
        files::copy_new(
            &scratch.path().join("original"),
            &scratch.path().join("target")
        )
        .is_err()
    );
    assert_eq!(fs::read(scratch.path().join("important")).unwrap(), b"keep");
}

#[test]
fn path_validation_rejects_windows_aliases_and_escapes() {
    let scratch = tempfile::tempdir().unwrap();
    for name in [
        "../escape",
        "C:/escape",
        "/escape",
        "runtime/../../escape",
        "runtime\\..\\escape",
        "runtime/file:stream",
        "runtime/NUL.txt",
        "runtime/COM1",
        "runtime/trailing.",
        "runtime/trailing ",
        "runtime//file",
        "runtime/fi?le",
    ] {
        assert!(files::entry_path(scratch.path(), name).is_err(), "{name}");
    }
    assert!(files::remove_owned(scratch.path(), scratch.path()).is_err());
    assert!(files::remove_owned(scratch.path().parent().unwrap(), scratch.path()).is_err());
}

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        archive
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

#[test]
fn unsafe_and_case_aliased_archive_entries_are_refused_before_extraction() {
    for entries in [
        vec![("../escape", b"bad".as_slice())],
        vec![
            ("runtime/Hard Time.exe", b"one".as_slice()),
            ("runtime/hard time.exe", b"two".as_slice()),
        ],
        vec![("Data/player.dat", b"bad".as_slice())],
    ] {
        let scratch = tempfile::tempdir().unwrap();
        assert!(
            payload::unpack(&archive(&entries), scratch.path(), &Cancellation::default()).is_err()
        );
        assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 0);
    }
}

#[test]
fn cancellation_stops_unpacking_and_copying() {
    let scratch = tempfile::tempdir().unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(
        payload::unpack(
            &archive(&[("manifest.json", b"{}")]),
            scratch.path(),
            &cancel
        )
        .is_err()
    );
    assert!(files::copy_tree(scratch.path(), &scratch.path().join("new"), &cancel).is_err());
    assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 0);
}

#[test]
fn corrupt_or_missing_payload_is_rejected() {
    use sha2::{Digest, Sha256};
    let scratch = tempfile::tempdir().unwrap();
    let exe = scratch.path().join("setup.exe");
    let body = b"package";
    let mut bytes = vec![0_u8; 64];
    bytes.extend(body);
    bytes.extend(payload::MAGIC);
    bytes.extend(64_u64.to_le_bytes());
    bytes.extend((body.len() as u64).to_le_bytes());
    bytes.extend(Sha256::digest(body));
    fs::write(&exe, &bytes).unwrap();
    assert_eq!(payload::bundled(&exe).unwrap(), body);
    bytes[64] ^= 1;
    fs::write(&exe, &bytes).unwrap();
    assert!(payload::bundled(&exe).is_err());
    fs::write(&exe, b"not packaged").unwrap();
    assert!(payload::bundled(&exe).is_err());
}

#[test]
fn shortcuts_update_in_place_without_overwriting_another_installation() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let root = scratch.path().join("native 猫");
    fs::create_dir(&root)?;
    let folders = [
        scratch.path().join("Desktop"),
        scratch.path().join("Programs"),
    ];
    platform::create_shortcuts(&root, Some(&folders))?;
    platform::create_shortcuts(&root, Some(&folders))?;
    for folder in &folders {
        assert_eq!(fs::read_dir(folder)?.count(), 1);
    }
    let path = folders[0].join("Hard Time (Native).lnk");
    let before = fs::read(&path)?;
    assert!(platform::create_shortcuts(&scratch.path().join("another"), Some(&folders)).is_err());
    assert_eq!(fs::read(path)?, before);
    Ok(())
}
