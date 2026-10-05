//! Errors — every failure has a *kind*, and every kind has an *exit code*.
//!
//! This is the contract scripts and agents branch on (also printed by
//! `hippo-task --help`, and documented in README.md / AGENTS.md):
//!
//! | exit | kind        | meaning                                                  |
//! |------|-------------|----------------------------------------------------------|
//! | 0    | —           | success                                                  |
//! | 1    | `io`        | the ledger couldn't be read, written, or locked          |
//! | 2    | `usage`     | invalid input: bad value, ambiguous id, nothing to do    |
//! | 3    | `not_found` | no task matches the id                                   |
//! | 4    | `conflict`  | the task's state refuses the action (leased, closed)     |
//! | 5    | `stale`     | the description changed since `--base`, and the edits conflict |
//!
//! Rust notes: an `enum` whose variants carry data is how Rust says "exactly
//! one of these failures happened". Callers `match` on it, and the compiler
//! checks that every variant is handled. Nothing in this crate panics on bad
//! input or a bad disk — failures are ordinary values returned in a `Result`.

use std::fmt;

/// Every way a `hippo-task` operation can fail.
#[derive(Debug)]
pub enum Error {
    /// Invalid input (exit 2): an empty title, an ambiguous id, a task blocking itself…
    Usage(String),
    /// No task matches the id the user typed (exit 3).
    NotFound(String),
    /// The task's current state refuses the action (exit 4): someone else
    /// holds the lease, or the task is closed.
    Conflict(String),
    /// The description changed since the caller's `--base`, and the two edits
    /// don't merge (exit 5). Nothing was written. The caller re-reads and retries.
    Stale(String),
    /// An I/O failure (exit 1). `context` says what we were doing, to which
    /// file — so the message is actionable, not just "permission denied".
    Io {
        context: String,
        source: std::io::Error,
    },
}

/// Shorthand used throughout the crate: `Result<T>` = `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Build an [`Error::Io`] with context.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io {
            context: context.into(),
            source,
        }
    }

    /// The process exit code for this failure (see the table above).
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::Io { .. } => 1,
            Error::Usage(_) => 2,
            Error::NotFound(_) => 3,
            Error::Conflict(_) => 4,
            Error::Stale(_) => 5,
        }
    }

    /// The machine-readable kind, as it appears in `--json` errors.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Io { .. } => "io",
            Error::Usage(_) => "usage",
            Error::NotFound(_) => "not_found",
            Error::Conflict(_) => "conflict",
            Error::Stale(_) => "stale",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(msg) | Error::Conflict(msg) | Error::Stale(msg) => f.write_str(msg),
            Error::NotFound(id) => write!(f, "no task matches '{id}' (see `hippo-task list`)"),
            Error::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
