//! Filesystem watching.
//!
//! The folder is expected to change underneath the server: Obsidian autosaves while the
//! user types, agents write several notes in a row, and in the reference setup Synology
//! Drive syncs it. Each arrives as a burst of events spread over a second or more, and a
//! rebuild is not free beyond its own walk: it empties the render cache and sends every
//! open tab a change event. So events are gathered per root until the folder goes quiet,
//! and a batch is rebuilt only if it changed something the index holds.

use crate::state::AppState;
use notify::event::{AccessKind, AccessMode, EventKind};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct Timing {
    /// How long a root must go without a relevant event before its batch is rebuilt.
    quiet: Duration,
    /// The longest a batch waits while events keep arriving, so a long sync does not hide
    /// its first files until it ends.
    max_wait: Duration,
}

/// A 400 ms window timed per event used to split one save or sync into several rebuilds.
/// Replaying the October logs, a 1 s window per root removed 38% of the NAS's rebuilds,
/// and an edit still reaches the browser within about a second of the save.
const QUIET: Duration = Duration::from_secs(1);
/// Long enough to gather a sync of a few dozen files into one rebuild, short enough that
/// the first of them shows up while the rest are still arriving.
const MAX_WAIT: Duration = Duration::from_secs(5);
const TIMING: Timing = Timing {
    quiet: QUIET,
    max_wait: MAX_WAIT,
};

/// Paths one burst touched. `rescan` means the OS dropped events and cannot say which.
#[derive(Debug, Default)]
struct Batch {
    paths: BTreeSet<String>,
    rescan: bool,
}

struct Pending {
    batch: Batch,
    first: Instant,
    last: Instant,
}

impl Pending {
    fn due(&self, timing: Timing) -> Instant {
        (self.last + timing.quiet).min(self.first + timing.max_wait)
    }
}

/// Start watching every root. Hold the returned watchers for the process lifetime:
/// dropping one ends its root's watch and its batching thread.
pub fn spawn(state: Arc<AppState>) -> anyhow::Result<Vec<RecommendedWatcher>> {
    let mut watchers = Vec::new();

    for root in &state.config.roots {
        if !root.path.exists() {
            tracing::warn!(root = %root.id, path = %root.path.display(), "root does not exist; not watching");
            continue;
        }

        let (sender, events) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(sender)?;
        watcher.watch(&root.path, RecursiveMode::Recursive)?;

        let state = state.clone();
        let root_id = root.id.clone();
        let root_path = root.path.clone();
        std::thread::Builder::new()
            .name(format!("watch-{}", root.id))
            .spawn(move || {
                gather(&root_path, &events, TIMING, |batch| {
                    reindex_if_changed(&state, &root_id, &root_path, batch);
                });
            })?;

        tracing::info!(root = %root.id, path = %root.path.display(), "watching for changes");
        watchers.push(watcher);
    }
    Ok(watchers)
}

/// Collect relevant events into batches and hand each to `flush` once the root has been
/// quiet for `timing.quiet`, or `timing.max_wait` after the batch began. Returns, flushing
/// what it holds, once the watcher feeding `events` is dropped.
fn gather(
    root: &Path,
    events: &Receiver<notify::Result<Event>>,
    timing: Timing,
    mut flush: impl FnMut(Batch),
) {
    let mut pending: Option<Pending> = None;
    loop {
        let received = match &pending {
            None => events.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(burst) => {
                events.recv_timeout(burst.due(timing).saturating_duration_since(Instant::now()))
            }
        };
        match received {
            Ok(event) => absorb(root, event, &mut pending),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        // Checked after every event, not only on a timeout: a steady stream never lets the
        // receive time out, and the cap is what still flushes it.
        if let Some(burst) = pending.take_if(|burst| Instant::now() >= burst.due(timing)) {
            flush(burst.batch);
        }
    }
    if let Some(burst) = pending {
        flush(burst.batch);
    }
}

/// Add one event to the pending batch. An event the index would ignore neither starts a
/// batch nor extends its quiet window: Obsidian rewrites `.obsidian/workspace.json` while
/// the user types, and that must not hold back the note being typed.
fn absorb(root: &Path, event: notify::Result<Event>, pending: &mut Option<Pending>) {
    let event = match event {
        Ok(event) => event,
        Err(error) => {
            tracing::warn!(%error, "watch error");
            return;
        }
    };
    let rescan = event.need_rescan();
    let paths: Vec<String> = if is_change(&event.kind) {
        event
            .paths
            .iter()
            .filter_map(|path| relative_if_relevant(root, path))
            .collect()
    } else {
        Vec::new()
    };
    if paths.is_empty() && !rescan {
        return;
    }

    let now = Instant::now();
    let burst = pending.get_or_insert_with(|| Pending {
        batch: Batch::default(),
        first: now,
        last: now,
    });
    burst.last = now;
    burst.batch.paths.extend(paths);
    burst.batch.rescan |= rescan;
}

/// Rebuild only for the paths that differ from the index. The rest — an attribute change,
/// a temporary file that came and went, the echo of a save this server already indexed —
/// cannot change what a rebuild would produce.
fn reindex_if_changed(state: &AppState, root_id: &str, root: &Path, batch: Batch) {
    let Some(index) = state.index(root_id) else {
        return;
    };
    let mut changed: Vec<String> = batch
        .paths
        .into_iter()
        .filter(|path| !index.matches_disk(root, path))
        .collect();
    if batch.rescan && changed.is_empty() {
        // Nothing names what changed, so the change is reported against the root.
        changed.push(String::new());
    }
    if changed.is_empty() {
        tracing::debug!(root = %root_id, "every changed path is as indexed; not reindexing");
        return;
    }
    // Info, not debug: one line per edit burst is cheap, and a stream of them with nobody
    // editing is the only visible sign of a rebuild feeding itself.
    tracing::info!(root = %root_id, count = changed.len(), "reindexing after change");
    // `origin: None` marks this as an external change, which is what tells the client it
    // did not cause it and should refresh.
    state.reindex(root_id, changed, None);
}

/// Whether an event can mean the file's content or name changed.
///
/// On Linux, notify subscribes to inotify's `IN_OPEN` and `IN_CLOSE_NOWRITE`, so merely
/// reading a file is reported. Rebuilding the index reads every file, so without this
/// filter each rebuild triggered the next: an endless reindex loop that held a core and
/// grew the NAS container to 9 GiB within two hours. macOS reports no reads, which is why
/// it never showed up there.
fn is_change(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

/// The root-relative path, or `None` for anything the index excludes.
///
/// Without the directory filter a Synology `@eaDir` thumbnail refresh would rebuild the
/// index repeatedly for content the app never shows.
fn relative_if_relevant(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    if kbviewer_core::paths::is_excluded(relative) {
        return None;
    }
    Some(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::scratch_state;
    use notify::event::{DataChange, Flag, ModifyKind};
    use std::path::PathBuf;
    use std::sync::mpsc;

    const ROOT: &str = "/vault";
    const FAST_QUIET: Duration = Duration::from_millis(100);
    const FAST_MAX_WAIT: Duration = Duration::from_millis(400);
    const FAST: Timing = Timing {
        quiet: FAST_QUIET,
        max_wait: FAST_MAX_WAIT,
    };
    /// Comfortably past `FAST.quiet`, so a flush that is due has happened.
    const SETTLE: Duration = Duration::from_millis(500);
    /// Longer than any test runs, so only the other bound can flush.
    const NEVER: Duration = Duration::from_secs(60);
    /// A steady stream sends this many events per quiet window, so the window never closes.
    const EVENTS_PER_QUIET_WINDOW: u32 = 4;
    /// A stream this many caps long must have been flushed by the cap more than once.
    const CAPS_TO_STREAM_FOR: u32 = 3;
    /// Ignored events keep arriving for this many quiet windows: had they extended the
    /// window, nothing would have been flushed by the end.
    const QUIET_WINDOWS_OF_NOISE: u32 = 5;
    /// A real-watcher test waits this many quiet windows, so its batch has been flushed
    /// and a second batch would have shown up too.
    const QUIET_WINDOWS_TO_SETTLE: u32 = 3;
    /// Between two saves of one burst: well inside the quiet window.
    const BETWEEN_SAVES: Duration = Duration::from_millis(50);

    fn write(path: &str) -> notify::Result<Event> {
        Ok(
            Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
                .add_path(PathBuf::from(format!("{ROOT}/{path}"))),
        )
    }

    /// `gather` on its own thread: send events in, receive the batches it flushes.
    fn gatherer(timing: Timing) -> (mpsc::Sender<notify::Result<Event>>, mpsc::Receiver<Batch>) {
        let (events_in, events) = mpsc::channel();
        let (flushed_out, flushed) = mpsc::channel();
        std::thread::spawn(move || {
            gather(Path::new(ROOT), &events, timing, |batch| {
                let _ = flushed_out.send(batch);
            });
        });
        (events_in, flushed)
    }

    fn paths(batch: &Batch) -> Vec<&str> {
        batch.paths.iter().map(String::as_str).collect()
    }

    #[test]
    fn a_burst_of_writes_is_one_batch() {
        let (events, flushed) = gatherer(FAST);
        for path in ["a.md", "b.md", "a.md", "c.md"] {
            events.send(write(path)).unwrap();
        }
        let batch = flushed.recv_timeout(SETTLE).expect("flushed once quiet");
        assert_eq!(paths(&batch), ["a.md", "b.md", "c.md"]);
        assert!(flushed.recv_timeout(SETTLE).is_err(), "and only once");
    }

    #[test]
    fn writes_further_apart_than_the_quiet_window_are_separate_batches() {
        let (events, flushed) = gatherer(FAST);
        events.send(write("a.md")).unwrap();
        assert_eq!(paths(&flushed.recv_timeout(SETTLE).unwrap()), ["a.md"]);
        events.send(write("b.md")).unwrap();
        assert_eq!(paths(&flushed.recv_timeout(SETTLE).unwrap()), ["b.md"]);
    }

    #[test]
    fn a_steady_stream_is_still_flushed_at_the_cap() {
        let (events, flushed) = gatherer(FAST);
        let started = Instant::now();
        let mut sent = 0;
        while started.elapsed() < FAST.max_wait * CAPS_TO_STREAM_FOR {
            events.send(write(&format!("cover-{sent}.jpg"))).unwrap();
            sent += 1;
            std::thread::sleep(FAST.quiet / EVENTS_PER_QUIET_WINDOW);
        }
        let while_streaming = flushed.try_iter().count();
        assert!(
            while_streaming >= 2,
            "flushed {while_streaming} times mid-stream"
        );
    }

    #[test]
    fn events_the_index_ignores_never_start_a_batch() {
        let (events, flushed) = gatherer(FAST);
        events.send(write(".obsidian/workspace.json")).unwrap();
        events
            .send(write("tools/__pycache__/kb.cpython-312.pyc"))
            .unwrap();
        let read = Event::new(EventKind::Access(AccessKind::Open(AccessMode::Any)))
            .add_path(PathBuf::from(format!("{ROOT}/a.md")));
        events.send(Ok(read)).unwrap();
        assert!(flushed.recv_timeout(SETTLE).is_err());
    }

    #[test]
    fn ignored_events_do_not_hold_back_a_real_change() {
        let timing = Timing {
            quiet: FAST.quiet,
            max_wait: NEVER,
        };
        let (events, flushed) = gatherer(timing);
        events.send(write("a.md")).unwrap();
        let started = Instant::now();
        while started.elapsed() < timing.quiet * QUIET_WINDOWS_OF_NOISE {
            events.send(write(".obsidian/workspace.json")).unwrap();
            std::thread::sleep(timing.quiet / EVENTS_PER_QUIET_WINDOW);
        }
        let batch = flushed
            .try_recv()
            .expect("flushed while ignored events kept arriving");
        assert_eq!(paths(&batch), ["a.md"]);
    }

    #[test]
    fn a_rescan_is_a_batch_even_without_paths() {
        let (events, flushed) = gatherer(FAST);
        events
            .send(Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan)))
            .unwrap();
        assert!(flushed.recv_timeout(SETTLE).unwrap().rescan);
    }

    #[test]
    fn dropping_the_watcher_flushes_what_is_pending() {
        let (events, flushed) = gatherer(Timing {
            quiet: NEVER,
            max_wait: NEVER,
        });
        events.send(write("a.md")).unwrap();
        drop(events);
        assert_eq!(paths(&flushed.recv_timeout(SETTLE).unwrap()), ["a.md"]);
    }

    fn batch_of(paths: &[&str]) -> Batch {
        Batch {
            paths: paths.iter().map(|path| path.to_string()).collect(),
            rescan: false,
        }
    }

    #[test]
    fn a_batch_that_changed_nothing_indexed_is_not_rebuilt() {
        let (state, root) = scratch_state("watch-noop");
        let mut changes = state.changes.subscribe();
        reindex_if_changed(&state, "kb", &root, batch_of(&["a.md", "gone.tmp"]));
        assert!(changes.try_recv().is_err());
    }

    /// Reporting the untouched path too would let a tab's own save echo hide someone
    /// else's edit in the same batch, since an event is attributed whole.
    #[test]
    fn only_the_paths_that_changed_are_reported() {
        let (state, root) = scratch_state("watch-changed");
        let mut changes = state.changes.subscribe();
        std::fs::write(root.join("b.md"), "# B\n").unwrap();
        reindex_if_changed(&state, "kb", &root, batch_of(&["a.md", "b.md"]));
        assert_eq!(changes.try_recv().unwrap().paths, ["b.md"]);
    }

    #[test]
    fn a_rescan_rebuilds_even_when_every_path_matches() {
        let (state, root) = scratch_state("watch-rescan");
        let mut changes = state.changes.subscribe();
        let batch = Batch {
            paths: BTreeSet::new(),
            rescan: true,
        };
        reindex_if_changed(&state, "kb", &root, batch);
        assert_eq!(changes.try_recv().unwrap().paths, [""]);
    }

    /// Wait out one full batch and return every change event it produced.
    fn events_after_one_batch(
        changes: &mut tokio::sync::broadcast::Receiver<kbviewer_core::model::ChangeEvent>,
    ) -> Vec<kbviewer_core::model::ChangeEvent> {
        std::thread::sleep(TIMING.quiet * QUIET_WINDOWS_TO_SETTLE);
        std::iter::from_fn(|| changes.try_recv().ok()).collect()
    }

    /// Long enough for the OS to have started delivering events for a new watch.
    const WATCH_STARTUP: Duration = Duration::from_millis(300);

    #[test]
    fn an_external_edit_burst_reaches_subscribers_once() {
        let (state, root) = scratch_state("watch-burst");
        let _watchers = spawn(state.clone()).unwrap();
        let mut changes = state.changes.subscribe();
        std::thread::sleep(WATCH_STARTUP);

        for body in ["# A\none\n", "# A\none two\n", "# A\none two three\n"] {
            std::fs::write(root.join("a.md"), body).unwrap();
            std::thread::sleep(BETWEEN_SAVES);
        }
        let events = events_after_one_batch(&mut changes);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].paths, ["a.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_permission_change_alone_sends_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let (state, root) = scratch_state("watch-chmod");
        let _watchers = spawn(state.clone()).unwrap();
        let mut changes = state.changes.subscribe();
        std::thread::sleep(WATCH_STARTUP);

        std::fs::set_permissions(root.join("a.md"), std::fs::Permissions::from_mode(0o600))
            .unwrap();
        assert!(events_after_one_batch(&mut changes).is_empty());
    }

    #[test]
    fn reports_ordinary_documents_and_ignores_excluded_ones() {
        let root = Path::new(ROOT);
        let relevant = |path: &str| relative_if_relevant(root, Path::new(path));
        assert_eq!(relevant("/vault/notes/a.md").as_deref(), Some("notes/a.md"));
        assert!(relevant("/vault/@eaDir/thumb.jpg").is_none());
        assert!(relevant("/vault/.obsidian/workspace.json").is_none());
        assert!(relevant("/vault/.trash/old.md").is_none());
        assert!(relevant("/elsewhere/a.md").is_none());
    }

    /// The reindex loop: rebuilding the index opens every file, and on Linux each open
    /// arrives as an event. Reads must never count as changes.
    #[test]
    fn reads_are_not_changes_but_a_closed_write_is() {
        assert!(!is_change(&EventKind::Access(AccessKind::Open(
            AccessMode::Any
        ))));
        assert!(!is_change(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(is_change(&EventKind::Access(AccessKind::Close(
            AccessMode::Write
        ))));
        assert!(is_change(&EventKind::Modify(ModifyKind::Any)));
    }
}
