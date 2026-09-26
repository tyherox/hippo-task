//! `hippo-task` — the open agent-native task schema as a small headless library,
//! plus a CLI (`src/main.rs`) for humans and coding agents.
//!
//! The one idea: **events are the source of truth; tasks are a projection.**
//! Every change is an immutable event appended to a single-file ledger; the
//! current state of every task is computed by folding those events.
//!
//! Read the code in this order:
//! 1. [`model`]  — the data: `Event` (source of truth) and `Task` (projection).
//! 2. [`fold`]   — **the heart**: how events become state (ordering, merge rules, leases).
//! 3. [`store`]  — the ledger file: locking, fsync, monotonic time, crash tolerance.
//! 4. [`ops`]    — the commands' rules: what's allowed, what's a conflict.
//! 5. [`error`]  — every failure has a kind and an exit code.
//! 6. [`render`] — the JSON shapes agents parse, and the text humans read.
//! 7. [`setup`]  — where a project's tasks live: `init`, finding the store, `guide`.
//!
//! `src/main.rs` is only argument parsing and printing.

pub mod error;
pub mod fold;
pub mod model;
pub mod ops;
pub mod render;
pub mod setup;
pub mod store;
