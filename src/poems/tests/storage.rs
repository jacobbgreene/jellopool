use super::*;
use crate::poems::storage::DraftStorage;
use std::fs;

#[test]
fn saves_all_drafts_active_selection_and_previous_good_backup() {
    let directory = TestDirectory::new();
    let storage = DraftStorage::open(directory.0.clone()).unwrap();
    assert!(storage.load().unwrap().book.is_none());
    let first = book();
    storage.save(&first).unwrap();
    let mut second = first.clone();
    second
        .add(PoemDocument::new(vec!["another".into()]).unwrap())
        .unwrap();
    storage.save(&second).unwrap();
    assert_eq!(storage.load().unwrap().book.unwrap(), second);
    let backup: DraftBook =
        ron::from_str(&fs::read_to_string(directory.0.join("drafts.ron.bak")).unwrap()).unwrap();
    assert_eq!(backup, first);
    drop(storage);
    assert_eq!(
        DraftStorage::open(directory.0.clone())
            .unwrap()
            .load()
            .unwrap()
            .book
            .unwrap(),
        second
    );
}

#[test]
fn damaged_primary_recovers_backup_and_preserves_damaged_bytes() {
    let directory = TestDirectory::new();
    let storage = DraftStorage::open(directory.0.clone()).unwrap();
    let book = book();
    storage.save(&book).unwrap();
    storage.save(&book).unwrap();
    fs::write(
        directory.0.join("drafts.ron"),
        b"interrupted or manually damaged",
    )
    .unwrap();
    let recovered = storage.load().unwrap();
    assert!(recovered.notice.is_some());
    assert_eq!(recovered.book.unwrap(), book);
    storage.save(&book).unwrap();
    let archive = fs::read_dir(&directory.0)
        .unwrap()
        .map(Result::unwrap)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("drafts.recovery-")
        })
        .unwrap();
    assert_eq!(
        fs::read(archive.path()).unwrap(),
        b"interrupted or manually damaged"
    );
}

#[test]
fn unknown_version_and_unrecoverable_corruption_are_never_silently_replaced() {
    let directory = TestDirectory::new();
    let storage = DraftStorage::open(directory.0.clone()).unwrap();
    let mut future = book();
    future.version = 999;
    let bytes = ron::to_string(&future).unwrap();
    fs::write(directory.0.join("drafts.ron"), &bytes).unwrap();
    // Even a valid backup must not downgrade a newer primary.
    fs::write(
        directory.0.join("drafts.ron.bak"),
        ron::to_string(&book()).unwrap(),
    )
    .unwrap();
    assert!(storage.load().is_err());
    assert!(storage.save(&book()).is_err());
    assert_eq!(
        fs::read_to_string(directory.0.join("drafts.ron")).unwrap(),
        bytes
    );
    fs::remove_file(directory.0.join("drafts.ron.bak")).unwrap();
    fs::write(directory.0.join("drafts.ron"), b"broken").unwrap();
    assert!(storage.load().is_err());
    assert_eq!(fs::read(directory.0.join("drafts.ron")).unwrap(), b"broken");
}

#[test]
fn second_writer_is_refused_and_failed_replace_leaves_previous_data() {
    let directory = TestDirectory::new();
    let storage = DraftStorage::open(directory.0.clone()).unwrap();
    assert!(DraftStorage::open(directory.0.clone()).is_err());
    storage.save(&book()).unwrap();
    let primary = fs::read(directory.0.join("drafts.ron")).unwrap();
    // A backup path that cannot be replaced makes the write fail before the
    // primary is touched. Deterministic even when tests run as root.
    fs::create_dir(directory.0.join("drafts.ron.bak")).unwrap();
    assert!(storage.save(&book()).is_err());
    assert_eq!(fs::read(directory.0.join("drafts.ron")).unwrap(), primary);
    assert!(
        !fs::read_dir(&directory.0)
            .unwrap()
            .map(Result::unwrap)
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
    );
}
