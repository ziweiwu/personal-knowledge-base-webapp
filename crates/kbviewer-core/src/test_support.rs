//! Fixture folders for tests that need a real directory to index.

use crate::config::RootConfig;
use std::path::Path;

/// Wipe what a previous run left at `dir`. Absent is the normal first-run case; any other
/// failure would leave stale files for the test to trip over, so it fails the test here.
pub(crate) fn remove_stale_dir(dir: &Path) {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("cannot clear {}: {error}", dir.display()),
    }
}

/// A scratch root under the temp dir holding `files`, wiped first so a rerun starts clean.
/// `wikilinks` is passed through as the config override; `None` exercises detection.
pub(crate) fn fixture_root(
    label: &str,
    files: &[(&str, &str)],
    wikilinks: Option<bool>,
) -> RootConfig {
    let root = std::env::temp_dir().join(format!("kbviewer-fixture-{label}"));
    remove_stale_dir(&root);
    std::fs::create_dir_all(&root).unwrap();
    for (path, body) in files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, body).unwrap();
    }
    RootConfig {
        id: "t".into(),
        name: "t".into(),
        path: root,
        index_names: vec!["index.md".into()],
        wikilinks,
        folder_notes: false,
        read_only: false,
    }
}
