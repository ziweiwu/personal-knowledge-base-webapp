//! Helpers shared by this crate's unit tests.

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
