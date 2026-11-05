//! バージョン履歴's store: each save a version, autosaves folded, the
//! original kept before the first save, the oldest let go.

use std::path::{Path, PathBuf};

use jade_office_model::versions::{VersionKind, VersionStore, KEEP, MERGE_WINDOW_SECS};

fn scratch(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("jade-versions-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&folder).ok();
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn saves_are_versions_newest_first_and_autosaves_fold() {
    let root = scratch("fold");
    let store = VersionStore::new(root.join("Versions"));
    let document = Path::new("/Users/me/Documents/契約書.docx");
    let first = store
        .record(document, b"one", VersionKind::Saved, 1_000)
        .unwrap()
        .unwrap();
    assert_eq!(first.kind, VersionKind::Saved);
    // The same bytes again make no new version.
    assert!(store
        .record(document, b"one", VersionKind::AutoSaved, 1_010)
        .unwrap()
        .is_none());
    store
        .record(document, b"two", VersionKind::AutoSaved, 1_020)
        .unwrap();
    // An autosave soon after another replaces it.
    store
        .record(document, b"three", VersionKind::AutoSaved, 1_080)
        .unwrap();
    let listed = store.list(document);
    assert_eq!(listed.len(), 2);
    assert_eq!(store.read(&listed[0]).unwrap(), b"three");
    assert_eq!(listed[0].kind, VersionKind::AutoSaved);
    assert_eq!(listed[1].kind, VersionKind::Saved);
    // Past the window it is a version of its own; a save always is.
    store
        .record(
            document,
            b"four",
            VersionKind::AutoSaved,
            1_080 + MERGE_WINDOW_SECS,
        )
        .unwrap();
    store
        .record(
            document,
            b"five",
            VersionKind::Saved,
            1_080 + MERGE_WINDOW_SECS,
        )
        .unwrap();
    let listed = store.list(document);
    assert_eq!(
        listed
            .iter()
            .map(|version| store.read(version).unwrap())
            .collect::<Vec<_>>(),
        vec![
            b"five".to_vec(),
            b"four".to_vec(),
            b"three".to_vec(),
            b"one".to_vec()
        ]
    );
    // Kept apart from the document, in the store's own folder.
    assert!(listed[0].file.starts_with(root.join("Versions")));
    assert_eq!(listed[0].file.extension().unwrap(), "docx");
    // Another document has a history of its own.
    assert!(store.list(Path::new("/Users/me/other.docx")).is_empty());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn the_original_is_kept_once_and_old_versions_go() {
    let root = scratch("original");
    let store = VersionStore::new(root.join("Versions"));
    let document = root.join("report.docx");
    // No file yet: nothing to keep.
    assert!(store.keep_original(&document, 10).unwrap().is_none());
    std::fs::write(&document, b"as opened").unwrap();
    let original = store.keep_original(&document, 20).unwrap().unwrap();
    assert_eq!(original.kind, VersionKind::Original);
    assert!(store.keep_original(&document, 30).unwrap().is_none());
    for n in 0..KEEP as u64 + 5 {
        store
            .record(
                &document,
                format!("save {n}").as_bytes(),
                VersionKind::Saved,
                100 + n,
            )
            .unwrap();
    }
    let listed = store.list(&document);
    assert_eq!(listed.len(), KEEP);
    assert_eq!(
        store.read(&listed[0]).unwrap(),
        format!("save {}", KEEP + 4).into_bytes()
    );
    assert!(listed
        .iter()
        .all(|version| version.kind != VersionKind::Original));
    std::fs::remove_dir_all(root).ok();
}
