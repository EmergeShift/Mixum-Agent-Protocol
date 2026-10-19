//! End-to-end tests for the `.mart` format: pack → open → verify → restore,
//! dedupe, base-archive reuse, diff and damage detection.

use map_snapshot::{format::validate_rel_path, pack, MartArchive, PackOptions, DEFAULT_CHUNK_SIZE};
use std::fs;
use std::io::Write;
use std::path::Path;

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = fs::File::create(path).unwrap();
    file.write_all(bytes).unwrap();
}

fn big_pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

#[test]
fn pack_open_verify_restore_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir_all(root.join("sub/nested")).unwrap();
    write_file(&root.join("a.txt"), b"hello mart\n");
    write_file(
        &root.join("sub/nested/b.bin"),
        &big_pattern(2 * 1024 * 1024 + 123),
    );
    write_file(&root.join("empty"), b"");
    #[cfg(unix)]
    std::os::unix::fs::symlink("a.txt", root.join("link")).unwrap();

    let mart = dir.path().join("snap.mart");
    let stats = pack(&root, &mart, &PackOptions::default()).unwrap();
    assert_eq!(stats.files, 3 + cfg!(unix) as u64);
    assert!(stats.dirs >= 2);
    // 2 MiB + 123 bytes at 1 MiB chunks → 3 refs for the big file alone.
    assert!(stats.chunk_refs >= 3);

    let archive = MartArchive::open(&mart).unwrap();
    let report = archive.verify().unwrap();
    assert!(report.ok);
    assert_eq!(report.files, stats.files);

    let manifest = archive.to_snapshot_json();
    let paths: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"a.txt"));
    assert!(paths.contains(&"sub/nested/b.bin"));
    assert!(paths.contains(&"empty"));

    let restored = dir.path().join("restored");
    let restore = archive.restore(&restored).unwrap();
    assert_eq!(restore.files, stats.files);
    assert_eq!(fs::read(restored.join("a.txt")).unwrap(), b"hello mart\n");
    assert_eq!(
        fs::read(restored.join("sub/nested/b.bin")).unwrap(),
        big_pattern(2 * 1024 * 1024 + 123)
    );
    assert_eq!(fs::read(restored.join("empty")).unwrap(), b"");
    #[cfg(unix)]
    {
        let target = fs::read_link(restored.join("link")).unwrap();
        assert_eq!(target.to_string_lossy(), "a.txt");
    }
}

#[test]
fn identical_files_share_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let payload = big_pattern(DEFAULT_CHUNK_SIZE / 2);
    write_file(&root.join("one.bin"), &payload);
    write_file(&root.join("two.bin"), &payload);
    let mart = dir.path().join("dedupe.mart");
    let stats = pack(&root, &mart, &PackOptions::default()).unwrap();
    assert_eq!(stats.chunk_refs, 2);
    assert_eq!(stats.chunks_stored, 1);
    assert_eq!(stats.chunks_reused, 1);
    MartArchive::open(&mart).unwrap().verify().unwrap();
}

#[test]
fn base_archive_reuses_unchanged_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let payload = big_pattern(3 * DEFAULT_CHUNK_SIZE + 7);
    write_file(&root.join("data.bin"), &payload);
    write_file(&root.join("keep.txt"), b"unchanged");

    let base = dir.path().join("base.mart");
    pack(&root, &base, &PackOptions::default()).unwrap();

    // Touch one byte in the first chunk, append a new file.
    let mut changed = payload.clone();
    changed[0] ^= 0xff;
    write_file(&root.join("data.bin"), &changed);
    write_file(&root.join("new.txt"), b"new file");

    let next = dir.path().join("next.mart");
    let options = PackOptions {
        base: Some(base.clone()),
        ..PackOptions::default()
    };
    let stats = pack(&root, &next, &options).unwrap();
    assert!(
        stats.chunks_reused >= 2,
        "expected base reuse, got {stats:?}"
    );

    let before = MartArchive::open(&base).unwrap();
    let after = MartArchive::open(&next).unwrap();
    after.verify().unwrap();
    let diff = before.diff(&after);
    assert!(diff.files_added.contains(&"new.txt".to_owned()));
    assert!(diff
        .files_modified
        .iter()
        .any(|entry| entry.path == "data.bin"));
}

#[test]
fn damage_is_detected_by_verify() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    write_file(&root.join("payload.bin"), &big_pattern(4096));
    let mart = dir.path().join("snap.mart");
    pack(&root, &mart, &PackOptions::default()).unwrap();

    let mut bytes = fs::read(&mart).unwrap();
    // First chunk payload starts right after the fixed header.
    let offset = map_snapshot::HEADER_LEN as usize + 3;
    bytes[offset] ^= 0xff;
    fs::write(&mart, &bytes).unwrap();

    let archive = MartArchive::open(&mart).unwrap();
    assert!(archive.verify().is_err(), "flipped chunk byte must fail");
}

#[test]
fn diff_reports_add_remove_modify() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    write_file(&first.join("keep.txt"), b"same");
    write_file(&first.join("gone.txt"), b"bye");
    write_file(&first.join("edit.txt"), b"old");
    let a = dir.path().join("a.mart");
    pack(&first, &a, &PackOptions::default()).unwrap();

    let second = dir.path().join("second");
    write_file(&second.join("keep.txt"), b"same");
    write_file(&second.join("edit.txt"), b"new");
    write_file(&second.join("added.txt"), b"hi");
    let b = dir.path().join("b.mart");
    pack(&second, &b, &PackOptions::default()).unwrap();

    let report = MartArchive::open(&a)
        .unwrap()
        .diff(&MartArchive::open(&b).unwrap());
    assert_eq!(report.files_added, vec!["added.txt".to_owned()]);
    assert_eq!(report.files_removed, vec!["gone.txt".to_owned()]);
    assert_eq!(report.files_modified.len(), 1);
    assert_eq!(report.files_modified[0].path, "edit.txt");
}

#[test]
fn manifest_paths_cannot_escape() {
    assert!(validate_rel_path("a/b").is_ok());
    assert!(validate_rel_path("../etc/passwd").is_err());
    assert!(validate_rel_path("/etc/passwd").is_err());
    assert!(validate_rel_path("a/../../b").is_err());
    assert!(validate_rel_path("").is_err());
}

#[test]
fn tampered_chunk_is_rejected_by_restore() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    write_file(&root.join("payload.bin"), &big_pattern(4096));
    let mart = dir.path().join("snap.mart");
    pack(&root, &mart, &PackOptions::default()).unwrap();
    let mut bytes = fs::read(&mart).unwrap();
    bytes[map_snapshot::HEADER_LEN as usize + 2] ^= 0xff;
    fs::write(&mart, &bytes).unwrap();
    let archive = MartArchive::open(&mart).unwrap();
    assert!(archive.verify().is_err());
    assert!(
        archive.restore(dir.path().join("out")).is_err(),
        "restore 必须默认校验 chunk"
    );
}
