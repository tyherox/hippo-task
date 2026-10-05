//! The single-file event ledger — the storage adapter.
//!
//! Layout: `<store folder>/ledger.jsonl` — one JSON event per line, append-only.
//! The store folder is usually `<project>/.hippotask`; `setup` finds it (ADR-005).
//!
//! Guarantees the rest of the code relies on:
//! 1. **Serialized writers.** Every write happens inside a [`Tx`] that holds an
//!    exclusive advisory lock across read → decide → append. Readers take a
//!    shared lock, so they never see half a line.
//! 2. **Durable appends.** A commit writes all of its lines in one `write_all`,
//!    then `sync_data`: when a command says it succeeded, the event is on disk.
//! 3. **Monotonic time.** Inside a transaction, each timestamp is
//!    `max(wall clock, newest ts in the ledger + 1)`. So on one machine,
//!    ledger order == time order — even for events in the same millisecond,
//!    even if the wall clock steps backwards. (This is the "physical" half of a
//!    Hybrid Logical Clock; the full HLC arrives with multi-machine sync.)
//! 4. **Crash tolerance.** A crash mid-write can leave a torn last line.
//!    Readers skip unreadable lines — *reporting* each one, never silently —
//!    and a writer first fences a torn tail with a newline so the torn bytes
//!    can't swallow the next good event.
//!
//! Deferred (staging rule): snapshots/compaction, other adapters (SQLite, sync).

use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::Event;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// How long a command waits for another `hippo-task` process to finish writing
/// before giving up with an `io` error (rather than hanging forever).
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Called with each warning (a skipped line, a directory that couldn't be
/// flushed). Default: print to stderr. (`Rc` so a transaction can share it.)
type WarnFn = Rc<dyn Fn(&str)>;

/// The ledger in one store folder.
pub struct Store {
    /// Must already exist, so a typo can't start a new, empty list somewhere
    /// else: the project folder for [`Store::new`], the store folder's parent
    /// for [`Store::in_folder`].
    base: PathBuf,
    /// Holds `ledger.jsonl`; created by the first write if it's missing.
    folder: PathBuf,
    path: PathBuf,
    lock_timeout: Duration,
    on_warning: WarnFn,
}

/// Everything read from the ledger.
#[derive(Debug, Default)]
pub struct Ledger {
    /// Every readable event, in file order.
    pub events: Vec<Event>,
    /// One message per line that couldn't be read (skipped, never silently).
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy)]
enum LockKind {
    Shared,
    Exclusive,
}

impl Store {
    /// A store for `<dir>/.hippotask/ledger.jsonl`. Nothing touches the disk until
    /// you read or write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let folder = dir.join(".hippotask");
        Self::with(dir, folder)
    }

    /// A store for `<folder>/ledger.jsonl` — a folder `hippo-task init` chose,
    /// wherever it is (ADR-005).
    pub fn in_folder(folder: impl Into<PathBuf>) -> Self {
        let folder = folder.into();
        let base = folder
            .parent()
            .map_or_else(|| folder.clone(), Path::to_path_buf);
        Self::with(base, folder)
    }

    fn with(base: PathBuf, folder: PathBuf) -> Self {
        let path = folder.join("ledger.jsonl");
        Store {
            base,
            folder,
            path,
            lock_timeout: DEFAULT_LOCK_TIMEOUT,
            on_warning: Rc::new(|w| eprintln!("warning: {w}")),
        }
    }

    /// Override how long to wait for the lock (tests use a short one).
    pub fn with_lock_timeout(mut self, timeout: Duration) -> Self {
        self.lock_timeout = timeout;
        self
    }

    /// Route skipped-line warnings somewhere else (the CLI emits JSON in `--json` mode).
    pub fn on_warning(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_warning = Rc::new(handler);
        self
    }

    /// Where the ledger file lives.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The store folder — the ledger's folder, where `config.toml` lives too.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// This store's settings (ADR-006). A store without a `config.toml` has
    /// an empty one, so this only fails on a file that can't be read or parsed.
    /// Settings this version doesn't know are reported as warnings — so load
    /// it once per command.
    pub fn config(&self) -> Result<Config> {
        let config = Config::load(&self.folder)?;
        for warning in &config.warnings {
            self.warn(warning);
        }
        Ok(config)
    }

    /// Report something worth knowing that isn't an error, through the same
    /// channel as skipped lines — so `--json` mode turns it into a JSON line.
    pub fn warn(&self, message: &str) {
        (self.on_warning)(message);
    }

    /// Read every event under a shared lock. A missing ledger is an empty one —
    /// and reading never creates anything.
    pub fn read(&self) -> Result<Ledger> {
        self.check_dir()?;
        let mut file = match File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Ledger::default()),
            Err(e) => {
                return Err(Error::io(
                    format!("couldn't open {}", self.path.display()),
                    e,
                ))
            }
        };
        self.lock(&file, LockKind::Shared)?;
        let bytes = self.read_bytes(&mut file)?;
        Ok(self.parse(&bytes))
    }

    /// Begin a write transaction: take the exclusive lock, then read the
    /// ledger as it is *now* — decisions made inside the transaction can't be
    /// invalidated by a concurrent writer.
    pub fn begin(&self) -> Result<Tx> {
        self.check_dir()?;
        let tasks_dir = self.folder.clone();
        let dir_is_new = !tasks_dir.is_dir();
        let file_is_new = !self.path.exists();
        fs::create_dir_all(&tasks_dir)
            .map_err(|e| Error::io(format!("couldn't create {}", tasks_dir.display()), e))?;
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&self.path)
            .map_err(|e| Error::io(format!("couldn't open {}", self.path.display()), e))?;
        self.lock(&file, LockKind::Exclusive)?;
        let bytes = self.read_bytes(&mut file)?;
        let torn_tail = bytes.last().is_some_and(|b| *b != b'\n');
        let ledger = self.parse(&bytes);
        let last_ts = ledger.events.iter().map(|e| e.ts).max();
        // A brand-new file (or folder) is only crash-safe once the directory
        // that lists it is flushed too — commit does that, once.
        let mut new_entries_in = Vec::new();
        if file_is_new {
            new_entries_in.push(tasks_dir);
        }
        if dir_is_new {
            new_entries_in.push(self.base.clone());
        }
        Ok(Tx {
            file,
            path: self.path.clone(),
            len: bytes.len() as u64,
            ledger,
            last_ts,
            torn_tail,
            new_entries_in,
            warn: Rc::clone(&self.on_warning),
        })
    }

    /// `--dir` must name an existing folder: a typo must not silently start a
    /// brand-new, empty task list somewhere else.
    fn check_dir(&self) -> Result<()> {
        if self.base.is_dir() {
            return Ok(());
        }
        Err(Error::Usage(format!(
            "no such directory: {} — point --dir / HIPPO_DIR at an existing folder",
            self.base.display()
        )))
    }

    /// Advisory lock with a deadline. Rust note: `loop` + `match` is the
    /// idiomatic retry; `?`-style early `return`s leave the loop.
    fn lock(&self, file: &File, kind: LockKind) -> Result<()> {
        let started = Instant::now();
        loop {
            let attempt = match kind {
                LockKind::Shared => file.try_lock_shared(),
                LockKind::Exclusive => file.try_lock(),
            };
            match attempt {
                Ok(()) => return Ok(()),
                Err(TryLockError::WouldBlock) if started.elapsed() < self.lock_timeout => {
                    sleep(Duration::from_millis(5));
                }
                Err(TryLockError::WouldBlock) => {
                    let why = std::io::Error::new(
                        ErrorKind::TimedOut,
                        format!(
                            "another `hippo-task` process held it for over {:?}",
                            self.lock_timeout
                        ),
                    );
                    return Err(Error::io(
                        format!("couldn't lock {}", self.path.display()),
                        why,
                    ));
                }
                Err(TryLockError::Error(e)) => {
                    return Err(Error::io(
                        format!("couldn't lock {}", self.path.display()),
                        e,
                    ));
                }
            }
        }
    }

    fn read_bytes(&self, file: &mut File) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| Error::io(format!("couldn't read {}", self.path.display()), e))?;
        Ok(bytes)
    }

    /// Parse raw bytes line by line. Works on bytes, not `String`s, so one
    /// invalid-UTF-8 line is skipped instead of making the whole ledger unreadable.
    fn parse(&self, bytes: &[u8]) -> Ledger {
        let mut ledger = Ledger::default();
        for (index, raw) in bytes.split(|b| *b == b'\n').enumerate() {
            let line = raw.trim_ascii();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_slice::<Event>(line) {
                Ok(event) => ledger.events.push(event),
                Err(e) => {
                    // An event kind this version doesn't know was written by a
                    // newer hippo-task — say so, rather than "unreadable".
                    let what = if e.to_string().contains("unknown variant") {
                        "skipped an event written by a newer hippo-task — upgrade hippo-task to read it"
                    } else {
                        "skipped an unreadable line"
                    };
                    let msg = format!("{}:{}: {what} ({e})", self.path.display(), index + 1);
                    (self.on_warning)(&msg);
                    ledger.warnings.push(msg);
                }
            }
        }
        ledger
    }
}

/// A write transaction. It holds the exclusive lock from [`Store::begin`]
/// until it is committed or dropped.
///
/// Rust note: the lock lives inside `file`; when the `File` is dropped, the OS
/// releases the lock. Tying a resource to a value's lifetime like this is
/// called RAII — the same pattern as `MutexGuard`.
pub struct Tx {
    file: File,
    path: PathBuf,
    /// File length when the lock was taken — where to roll back to on failure.
    len: u64,
    ledger: Ledger,
    last_ts: Option<i64>,
    torn_tail: bool,
    /// Directories holding an entry this transaction created (flushed on commit).
    new_entries_in: Vec<PathBuf>,
    warn: WarnFn,
}

impl Tx {
    /// The ledger as it was when the lock was taken.
    pub fn events(&self) -> &[Event] {
        &self.ledger.events
    }

    /// Timestamp for the next event: `max(now, newest + 1)`. Strictly
    /// increasing, so within this ledger no two events share a timestamp and
    /// append order is time order.
    pub fn next_ts(&mut self, now_ms: i64) -> i64 {
        let ts = match self.last_ts {
            Some(last) if last >= now_ms => last.saturating_add(1),
            _ => now_ms,
        };
        self.last_ts = Some(ts);
        ts
    }

    /// Append `new` durably (one write, then fsync), release the lock, and
    /// return the whole ledger — old + new — so callers can fold the result
    /// without reading the file again.
    ///
    /// All or nothing: if the write or the flush fails, the file is cut back to
    /// its length before this commit, so a failed command never leaves half a
    /// change behind (and retrying can't duplicate it).
    pub fn commit(mut self, new: Vec<Event>) -> Result<Ledger> {
        if new.is_empty() {
            return Ok(self.ledger);
        }
        let mut buf = Vec::new();
        if self.torn_tail {
            buf.push(b'\n'); // fence off a torn last line from a crashed writer
        }
        for event in &new {
            serde_json::to_writer(&mut buf, event).map_err(|e| {
                Error::io(
                    format!("couldn't encode an event for {}", self.path.display()),
                    e.into(),
                )
            })?;
            buf.push(b'\n');
        }
        let written = self
            .file
            .write_all(&buf)
            .map_err(|e| ("append to", e))
            .and_then(|()| self.file.sync_data().map_err(|e| ("flush", e)));
        if let Err((what, source)) = written {
            let path = self.path.display();
            let rolled_back = self
                .file
                .set_len(self.len)
                .and_then(|()| self.file.sync_data());
            let context = match rolled_back {
                Ok(()) => format!("couldn't {what} {path} — rolled back, nothing was recorded"),
                Err(e) => format!(
                    "couldn't {what} {path}, and rolling back failed too ({e}) — part of this change may be recorded; check `hippo-task show` before retrying"
                ),
            };
            return Err(Error::io(context, source));
        }
        for dir in &self.new_entries_in {
            if let Err(e) = flush_dir(dir) {
                // Best effort (some filesystems can't flush a directory):
                // report it, but the events themselves are already on disk.
                (self.warn)(&format!(
                    "couldn't flush the folder {} to disk ({e}); the ledger is written, but a power cut right now could lose the new file",
                    dir.display()
                ));
            }
        }
        self.ledger.events.extend(new);
        Ok(self.ledger)
    }
}

/// Flush a folder's list of entries to disk, so a file just created in it
/// survives a power cut.
///
/// Rust note: `#[cfg(...)]` picks one of these two functions at compile time,
/// so each platform's binary only contains its own version.
#[cfg(not(windows))]
fn flush_dir(dir: &Path) -> std::io::Result<()> {
    File::open(dir).and_then(|d| d.sync_all())
}

/// Windows can't open a folder as a plain file, so there's nothing to flush
/// here; the ledger file itself was already flushed. (SQLite skips this step
/// on Windows too.)
#[cfg(windows)]
fn flush_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}
