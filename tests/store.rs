//! The ledger file's guarantees (store.rs): durability, locking, monotonic
//! time, and crash tolerance — tested through the library API.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::TempDir;
use hippo_task::error::Error;
use hippo_task::model::{Event, EventKind, Priority};
use hippo_task::store::Store;
use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn event(eid: &str, task: &str, ts: i64) -> Event {
    Event {
        eid: eid.into(),
        task: task.into(),
        ts,
        actor: "agent:test".into(),
        node: "n1".into(),
        kind: EventKind::Create {
            title: format!("task {task}"),
            priority: Priority::None,
            body: None,
            assignee: None,
        },
    }
}

#[test]
fn creating_a_store_is_quiet() {
    // The first write creates the folder and the ledger, then flushes the
    // folders that list them to disk. That has to work on every platform —
    // Windows included, where a folder can't be opened like a file — without
    // a warning on a perfectly normal first write.
    let dir = TempDir::new("store-create-quiet");
    let warnings = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = Rc::clone(&warnings);
    let store = Store::new(dir.path()).on_warning(move |w| sink.borrow_mut().push(w.to_string()));
    store
        .begin()
        .unwrap()
        .commit(vec![event("E1", "T1", 1)])
        .unwrap();
    assert_eq!(store.read().unwrap().events.len(), 1);
    assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
}

#[test]
fn committed_events_read_back_identically() {
    let dir = TempDir::new("store-roundtrip");
    let store = Store::new(dir.path());
    let written = vec![event("E1", "T1", 10), event("E2", "T2", 11)];
    let tx = store.begin().unwrap();
    tx.commit(written.clone()).unwrap();

    let ledger = store.read().unwrap();
    assert_eq!(ledger.events, written);
    assert!(ledger.warnings.is_empty());
}

#[test]
fn reading_creates_nothing_and_a_missing_ledger_is_empty() {
    let dir = TempDir::new("store-read-empty");
    let ledger = Store::new(dir.path()).read().unwrap();
    assert!(ledger.events.is_empty());
    assert!(
        !dir.path().join(".hippotask").exists(),
        "a read must not create .hippotask/"
    );
}

#[test]
fn a_missing_directory_is_a_usage_error_and_is_never_created() {
    let dir = TempDir::new("store-missing-dir");
    let typo = dir.path().join("no").join("such").join("dir");
    let store = Store::new(&typo);
    assert!(matches!(store.read(), Err(Error::Usage(_))));
    assert!(matches!(store.begin(), Err(Error::Usage(_))));
    assert!(
        !typo.exists(),
        "a typo'd --dir must not start a new task list"
    );
}

#[test]
fn timestamps_strictly_increase_even_when_the_clock_goes_backwards() {
    let dir = TempDir::new("store-monotonic");
    let store = Store::new(dir.path());

    let mut tx = store.begin().unwrap();
    let a = tx.next_ts(1_000);
    let b = tx.next_ts(1_000); // same millisecond
    let c = tx.next_ts(500); // clock stepped back
    let d = tx.next_ts(5_000); // clock jumped ahead
    assert_eq!((a, b, c, d), (1_000, 1_001, 1_002, 5_000));
    tx.commit(vec![event("E1", "T1", d)]).unwrap();

    // A later transaction continues from the newest timestamp on disk.
    let mut tx = store.begin().unwrap();
    assert_eq!(tx.next_ts(10), 5_001);
}

#[test]
fn a_torn_last_line_cannot_swallow_the_next_event() {
    let dir = TempDir::new("store-torn");
    let store = Store::new(dir.path());
    store
        .begin()
        .unwrap()
        .commit(vec![event("E1", "T1", 1)])
        .unwrap();

    // Simulate a crash mid-write: half a line, no newline.
    let mut f = OpenOptions::new().append(true).open(store.path()).unwrap();
    f.write_all(br#"{"eid":"E2","task":"T2","ts":2,"act"#)
        .unwrap();
    drop(f);

    store
        .begin()
        .unwrap()
        .commit(vec![event("E3", "T3", 3)])
        .unwrap();

    let ledger = store.read().unwrap();
    let eids: Vec<&str> = ledger.events.iter().map(|e| e.eid.as_str()).collect();
    assert_eq!(
        eids,
        ["E1", "E3"],
        "the event after the torn line must survive"
    );
    assert_eq!(
        ledger.warnings.len(),
        1,
        "the torn line is reported, not hidden"
    );
    assert!(
        ledger.warnings[0].contains(":2:"),
        "warning names the line: {:?}",
        ledger.warnings
    );
}

#[test]
fn unreadable_lines_are_skipped_and_reported_never_fatal() {
    let dir = TempDir::new("store-garbage");
    let store = Store::new(dir.path());
    store
        .begin()
        .unwrap()
        .commit(vec![event("E1", "T1", 1)])
        .unwrap();

    let mut f = OpenOptions::new().append(true).open(store.path()).unwrap();
    f.write_all(b"\xff\xfe not utf-8\n").unwrap();
    f.write_all(b"{\"not\": \"an event\"}\n").unwrap();
    f.write_all(b"\n   \n").unwrap(); // blank lines are fine, not warnings
    drop(f);
    store
        .begin()
        .unwrap()
        .commit(vec![event("E2", "T2", 2)])
        .unwrap();

    let ledger = store.read().unwrap();
    assert_eq!(ledger.events.len(), 2);
    assert_eq!(ledger.warnings.len(), 2, "{:?}", ledger.warnings);
}

#[test]
fn warnings_go_to_the_configured_handler() {
    let dir = TempDir::new("store-warn-handler");
    fs::create_dir_all(dir.path().join(".hippotask")).unwrap();
    fs::write(dir.ledger(), "garbage\n").unwrap();

    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let sink = seen.clone();
    let store = Store::new(dir.path()).on_warning(move |w| sink.borrow_mut().push(w.to_string()));
    store.read().unwrap();
    assert_eq!(seen.borrow().len(), 1);
}

#[test]
fn a_held_lock_times_out_with_an_io_error_instead_of_hanging() {
    let dir = TempDir::new("store-lock");
    let store = Store::new(dir.path()).with_lock_timeout(Duration::from_millis(200));
    store
        .begin()
        .unwrap()
        .commit(vec![event("E1", "T1", 1)])
        .unwrap();

    // Another "process" holds the exclusive lock (a separate open file = a separate lock owner).
    let holder = File::open(store.path()).unwrap();
    holder.lock().unwrap();

    let started = Instant::now();
    let err = store.begin().err().expect("must not get the lock");
    assert!(matches!(err, Error::Io { .. }), "{err:?}");
    assert_eq!(err.exit_code(), 1);
    assert!(err.to_string().contains("couldn't lock"), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "gave up promptly"
    );
    assert!(store.read().is_err(), "readers wait for writers too");

    // Once released, everything works again.
    holder.unlock().unwrap();
    assert!(store.begin().is_ok());
}

#[test]
fn an_empty_commit_writes_nothing() {
    let dir = TempDir::new("store-empty-commit");
    let store = Store::new(dir.path());
    store.begin().unwrap().commit(vec![]).unwrap();
    assert_eq!(fs::read(store.path()).unwrap().len(), 0);
}

#[test]
fn the_first_write_in_a_new_folder_is_flushed_without_warnings() {
    // Creating .hippotask/ and the ledger adds directory entries; commit flushes them too.
    let dir = TempDir::new("store-first-write");
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let sink = seen.clone();
    let store = Store::new(dir.path()).on_warning(move |w| sink.borrow_mut().push(w.to_string()));
    store
        .begin()
        .unwrap()
        .commit(vec![event("E1", "T1", 1)])
        .unwrap();
    assert!(seen.borrow().is_empty(), "{:?}", seen.borrow());
    assert_eq!(store.read().unwrap().events.len(), 1);
}
