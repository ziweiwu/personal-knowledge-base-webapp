//! Fixture folders for tests that need a real directory to index.

use crate::config::RootConfig;

/// A scratch root under the temp dir holding `files`, wiped first so a rerun starts clean.
/// `wikilinks` is passed through as the config override; `None` exercises detection.
pub(crate) fn fixture_root(
    label: &str,
    files: &[(&str, &str)],
    wikilinks: Option<bool>,
) -> RootConfig {
    let root = std::env::temp_dir().join(format!("kbviewer-fixture-{label}"));
    let _ = std::fs::remove_dir_all(&root);
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
