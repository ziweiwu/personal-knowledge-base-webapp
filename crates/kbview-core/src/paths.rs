//! Containment for every client-supplied path.
//!
//! Each filesystem read and write in the server resolves through [`resolve_in_root`].
//! It is the only thing standing between a request path and the rest of the disk, so it
//! is deliberately conservative: unknown shapes are rejected rather than normalised into
//! something plausible.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum PathError {
    /// The path escaped its root, or tried to.
    OutsideRoot(String),
    /// The path contained a component that is never legitimate in a request.
    InvalidComponent(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutsideRoot(path) => {
                write!(
                    formatter,
                    "refusing to access {path:?}: outside the vault root"
                )
            }
            Self::InvalidComponent(path) => {
                write!(
                    formatter,
                    "refusing to access {path:?}: invalid path component"
                )
            }
        }
    }
}

impl std::error::Error for PathError {}

/// Collapse `.` and `..` textually, refusing any path that would climb above the root.
///
/// This runs before touching the filesystem so that an escape attempt never becomes a
/// syscall. `..` is rejected outright rather than clamped: clamping silently turns a
/// hostile path into a valid one, which makes the logs lie about what was requested.
fn normalise(relative: &str) -> Result<PathBuf, PathError> {
    let reject = || PathError::InvalidComponent(relative.to_string());

    if relative.contains('\0') {
        return Err(reject());
    }

    let mut out = PathBuf::new();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => return Err(PathError::OutsideRoot(relative.to_string())),
            Component::RootDir | Component::Prefix(_) => {
                return Err(PathError::OutsideRoot(relative.to_string()))
            }
        }
    }
    Ok(out)
}

/// Resolve `relative` inside `root`, or fail.
///
/// The textual check above stops `..` and absolute paths. This additionally canonicalises
/// so that a **symlink** pointing out of the vault is caught too. Because the target may
/// legitimately not exist yet (a note being created), the nearest existing ancestor is
/// canonicalised and the missing tail re-appended — that still resolves every symlink on
/// the path that actually exists.
pub fn resolve_in_root(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    let normalised = normalise(relative)?;
    let joined = root.join(&normalised);

    let real_root = root
        .canonicalize()
        .map_err(|_| PathError::OutsideRoot(relative.to_string()))?;
    let real_target = canonicalise_nearest_existing(&joined);

    if !real_target.starts_with(&real_root) {
        return Err(PathError::OutsideRoot(relative.to_string()));
    }
    Ok(joined)
}

fn canonicalise_nearest_existing(target: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut current = target.to_path_buf();

    loop {
        if let Ok(real) = current.canonicalize() {
            missing.reverse();
            return missing.iter().fold(real, |acc, part| acc.join(part));
        }
        let Some(name) = current.file_name().map(|n| n.to_owned()) else {
            return target.to_path_buf();
        };
        missing.push(name);
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return target.to_path_buf(),
        }
    }
}

/// Directories that are never part of the browsable document set.
///
/// `@eaDir` and `.SynologyWorkingDirectory` are Synology sync artefacts; a vault living in
/// SynologyDrive is full of them and they must not appear as content.
const EXCLUDED_DIRS: &[&str] = &[
    ".obsidian",
    ".trash",
    ".git",
    ".svn",
    "@eaDir",
    ".SynologyWorkingDirectory",
    "node_modules",
];

pub fn is_excluded(relative: &Path) -> bool {
    relative.components().any(|component| {
        let Component::Normal(part) = component else {
            return false;
        };
        let name = part.to_string_lossy();
        EXCLUDED_DIRS.contains(&name.as_ref()) || name.starts_with('.') || name == "Icon\r"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kbview-paths-{label}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/note.md"), "hi").unwrap();
        dir
    }

    #[test]
    fn accepts_a_plain_relative_path() {
        let root = temp_root("plain");
        let resolved = resolve_in_root(&root, "sub/note.md").unwrap();
        assert_eq!(resolved, root.join("sub/note.md"));
    }

    #[test]
    fn accepts_a_path_that_does_not_exist_yet() {
        let root = temp_root("new");
        let resolved = resolve_in_root(&root, "sub/brand-new.md").unwrap();
        assert_eq!(resolved, root.join("sub/brand-new.md"));
    }

    #[test]
    fn rejects_parent_traversal() {
        let root = temp_root("traversal");
        for attempt in [
            "../outside.md",
            "sub/../../outside.md",
            "../../etc/passwd",
            "..",
        ] {
            assert!(
                matches!(
                    resolve_in_root(&root, attempt),
                    Err(PathError::OutsideRoot(_))
                ),
                "should have rejected {attempt:?}"
            );
        }
    }

    #[test]
    fn rejects_absolute_paths() {
        let root = temp_root("absolute");
        for attempt in ["/etc/passwd", "/", "//etc/passwd"] {
            assert!(
                resolve_in_root(&root, attempt).is_err(),
                "should have rejected {attempt:?}"
            );
        }
    }

    #[test]
    fn rejects_embedded_nul() {
        let root = temp_root("nul");
        assert!(matches!(
            resolve_in_root(&root, "sub/note.md\0.png"),
            Err(PathError::InvalidComponent(_))
        ));
    }

    #[test]
    fn tolerates_redundant_current_dir_segments() {
        let root = temp_root("curdir");
        let resolved = resolve_in_root(&root, "./sub/./note.md").unwrap();
        assert_eq!(resolved, root.join("sub/note.md"));
    }

    #[test]
    fn rejects_a_symlink_pointing_out_of_the_root() {
        let root = temp_root("symlink");
        let outside = std::env::temp_dir().join("kbview-paths-symlink-outside");
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.md"), "secret").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();

        assert!(
            matches!(
                resolve_in_root(&root, "escape/secret.md"),
                Err(PathError::OutsideRoot(_))
            ),
            "a symlink out of the vault must not be followed"
        );
    }

    #[test]
    fn excludes_sync_and_tool_artefacts() {
        assert!(is_excluded(Path::new(".obsidian/appearance.json")));
        assert!(is_excluded(Path::new("notes/@eaDir/thumb.jpg")));
        assert!(is_excluded(Path::new(".trash/old.md")));
        assert!(!is_excluded(Path::new("notes/real-note.md")));
    }
}
