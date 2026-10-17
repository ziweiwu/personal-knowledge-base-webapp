//! Where the server's own log goes, and how large it may grow.
//!
//! Stderr by default, which is right under Docker: the json-file driver rotates it. launchd
//! has no such driver — it appends to `StandardErrorPath` for ever, and an agent caught in
//! a restart loop once grew that file to 590 MB — so `--log-file` has the server write and
//! rotate the file itself. KBViewer.app passes it too, so the two share one implementation.

use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing_subscriber::EnvFilter;

const DEFAULT_FILTER: &str = "kbviewer=info,tower_http=warn";

/// The cap the Docker deployment gives its json-file driver, so the two bound alike.
const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
/// Older logs kept beside the live one, as `<file>.1` (newest) to `<file>.3`.
const KEPT_LOG_FILES: u32 = 3;

/// Sends the log to `log_file` when one is given, and to stderr otherwise.
pub fn init(log_file: Option<&Path>) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| DEFAULT_FILTER.into());
    let Some(path) = log_file else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
        return Ok(());
    };
    let file = RotatingFile::open(path, MAX_LOG_BYTES, KEPT_LOG_FILES)
        .with_context(|| format!("could not open the log file {}", path.display()))?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(Mutex::new(file))
        .init();
    record_panics();
    Ok(())
}

/// Under launchd nothing reads stderr, so a panic in a request handler has to reach the
/// log file or nobody ever sees it.
fn record_panics() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("{info}");
        default_hook(info);
    }));
}

/// A file that moves itself aside once it holds `max_bytes`, keeping the `kept` newest
/// predecessors and letting the oldest go. One file belongs to one server process.
pub struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    kept: u32,
    file: File,
    written: u64,
}

impl RotatingFile {
    /// Appends to what is already there, and counts it: a server restarted every ten
    /// seconds would otherwise start each life at zero and never rotate at all.
    pub fn open(path: &Path, max_bytes: u64, kept: u32) -> io::Result<Self> {
        let file = open_for_append(path)?;
        let written = file.metadata()?.len();
        Ok(Self {
            path: path.to_path_buf(),
            max_bytes,
            kept,
            file,
            written,
        })
    }

    /// Rotates before a write that would cross the cap rather than after, so a line is
    /// never split across two files. A line longer than the cap still goes in whole.
    fn would_overflow(&self, incoming: usize) -> bool {
        self.written > 0 && self.written + incoming as u64 > self.max_bytes
    }

    fn rotate(&mut self) -> io::Result<()> {
        for generation in (1..self.kept).rev() {
            rename_if_present(&self.rotated(generation), &self.rotated(generation + 1))?;
        }
        rename_if_present(&self.path, &self.rotated(1))?;
        self.file = open_for_append(&self.path)?;
        self.written = 0;
        Ok(())
    }

    fn rotated(&self, generation: u32) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(format!(".{generation}"));
        PathBuf::from(name)
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // A failed rotation must not cost the line: keep appending to the file in hand,
        // and try again only after another full file's worth.
        if self.would_overflow(buf.len()) && self.rotate().is_err() {
            self.written = 0;
        }
        let count = self.file.write(buf)?;
        self.written += count as u64;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn open_for_append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Renaming over an existing file replaces it, which is how the oldest log is let go.
fn rename_if_present(source: &Path, destination: &Path) -> io::Result<()> {
    match std::fs::rename(source, destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        outcome => outcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: u64 = 100;
    const KEPT: u32 = 2;
    const LINE: &[u8] = &[b'x'; 60];
    const LINES_WRITTEN: u32 = 5;

    fn read(path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn generation(path: &Path, number: u32) -> PathBuf {
        PathBuf::from(format!("{}.{number}", path.display()))
    }

    #[test]
    fn a_log_that_would_pass_its_cap_moves_aside_and_starts_afresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.log");
        let mut log = RotatingFile::open(&path, CAP, KEPT).unwrap();

        log.write_all(b"first line\n").unwrap();
        log.write_all(LINE).unwrap();
        log.write_all(b"after the cap\n").unwrap();
        log.write_all(LINE).unwrap();

        let previous = read(&generation(&path, 1)).unwrap();
        assert!(previous.starts_with("first line\n"), "{previous}");
        assert!(previous.ends_with("after the cap\n"), "{previous}");
        assert_eq!(read(&path).unwrap().as_bytes(), LINE);
    }

    #[test]
    fn only_the_newest_old_logs_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.log");
        let mut log = RotatingFile::open(&path, CAP, KEPT).unwrap();

        // Each line fills more than half the cap, so every write after the first rotates.
        for number in 1..=LINES_WRITTEN {
            let line = format!("{number:>width$}\n", width = LINE.len() - 1);
            log.write_all(line.as_bytes()).unwrap();
        }

        let holds = |path: &Path, number: u32| read(path).unwrap().trim() == number.to_string();
        assert!(holds(&path, LINES_WRITTEN));
        assert!(holds(&generation(&path, 1), LINES_WRITTEN - 1));
        assert!(holds(&generation(&path, KEPT), LINES_WRITTEN - KEPT));
        assert_eq!(read(&generation(&path, KEPT + 1)), None);
    }

    #[test]
    fn a_restarted_server_counts_what_its_log_already_holds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.log");
        std::fs::write(&path, LINE).unwrap();

        let mut log = RotatingFile::open(&path, CAP, KEPT).unwrap();
        log.write_all(LINE).unwrap();

        assert_eq!(read(&path).unwrap().len(), LINE.len());
        assert_eq!(read(&generation(&path, 1)).unwrap().len(), LINE.len());
    }

    #[test]
    fn a_line_longer_than_the_cap_is_written_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.log");
        let mut log = RotatingFile::open(&path, CAP, KEPT).unwrap();
        let long_line = vec![b'y'; CAP as usize + LINE.len()];

        log.write_all(&long_line).unwrap();

        assert_eq!(read(&path).unwrap().len(), long_line.len());
        assert_eq!(read(&generation(&path, 1)), None);
    }
}
