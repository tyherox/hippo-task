//! Shared helpers for the integration tests (each `tests/*.rs` file is its own
//! crate and includes this with `mod common;`).
// Not every test file uses every helper; and test helpers may panic — that's
// how a test fails (product code may not: see Cargo.toml [lints]).
#![allow(dead_code, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// A fresh, empty directory under the system temp dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("hippo-task-test-{tag}-{}-{n}", std::process::id()));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("clear a stale temp dir");
        }
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn ledger(&self) -> PathBuf {
        self.0.join(".hippotask").join("ledger.jsonl")
    }

    pub fn ledger_text(&self) -> String {
        std::fs::read_to_string(self.ledger()).expect("read ledger")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!(
                "warning: couldn't remove test dir {}: {e}",
                self.0.display()
            );
        }
    }
}

/// The result of one CLI invocation.
#[derive(Debug)]
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// Parse stdout as JSON (fails the test with the full output otherwise).
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{self:#?}"))
    }

    /// Parse the last stderr line as JSON (the `--json` error line).
    pub fn json_error(&self) -> serde_json::Value {
        let last = self.stderr.lines().last().unwrap_or_default();
        serde_json::from_str(last)
            .unwrap_or_else(|e| panic!("stderr is not JSON ({e}):\n{self:#?}"))
    }
}

/// The real binary with no HIPPO_* settings inherited from the developer's shell.
pub fn bare() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_hippo-task"));
    c.env_remove("HIPPO_DIR")
        .env_remove("HIPPO_ACTOR")
        .env_remove("HIPPO_NODE");
    c
}

/// A command for the real binary, pointed at `dir`, acting as `who` = (actor, node).
pub fn cmd(dir: &Path, who: (&str, &str), args: &[&str]) -> Command {
    let mut c = bare();
    c.args(args)
        .env("HIPPO_DIR", dir)
        .env("HIPPO_ACTOR", who.0)
        .env("HIPPO_NODE", who.1);
    c
}

/// Run the real binary and capture everything.
pub fn tasks(dir: &Path, who: (&str, &str), args: &[&str]) -> Run {
    let out = cmd(dir, who, args)
        .output()
        .expect("run the hippo-task binary");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Like `tasks` (above), but the command must succeed.
pub fn ok(dir: &Path, who: (&str, &str), args: &[&str]) -> Run {
    let run = tasks(dir, who, args);
    assert_eq!(run.code, 0, "expected success for {args:?}:\n{run:#?}");
    run
}

pub const HUMAN: (&str, &str) = ("human:tester", "n0");
pub const CLAUDE: (&str, &str) = ("agent:claude", "cc");
pub const CODEX: (&str, &str) = ("agent:codex", "cx");
