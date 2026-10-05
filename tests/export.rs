//! Upload to Notion (ADR-007): `hippo-task export notion` writes the CSV that
//! Notion imports — and, when it writes a file, remembers what it exported so
//! the next upload doesn't duplicate it.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{ok, tasks, TempDir, CLAUDE, HUMAN};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const CONFIG: &str = r#"
[fields.project]
values = ["dashboard", "billing"]

[fields.team]
values = ["design"]
"#;

fn with_fields(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    let store = dir.path().join(".hippotask");
    fs::create_dir_all(&store).unwrap();
    fs::write(store.join("config.toml"), CONFIG).unwrap();
    dir
}

/// CSV read the way an importer reads it (RFC 4180): quoted cells may hold
/// commas, newlines and doubled quotes. Written here on its own, so a quoting
/// bug in the export can't hide behind a matching bug in the parser.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                cell.push('"');
            }
            (true, '"') => quoted = false,
            (true, c) => cell.push(c),
            (false, '"') => quoted = true,
            (false, ',') => row.push(std::mem::take(&mut cell)),
            (false, '\r') => {}
            (false, '\n') => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            (false, c) => cell.push(c),
        }
    }
    assert!(!quoted, "unterminated quote in:\n{text}");
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

/// The value in `column` of data row `n` (1 = the first task).
fn cell<'a>(rows: &'a [Vec<String>], n: usize, column: &str) -> &'a str {
    let at = rows[0]
        .iter()
        .position(|h| h == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", rows[0]));
    &rows[n][at]
}

fn preview(dir: &Path, extra: &[&str]) -> Vec<Vec<String>> {
    let mut args = vec!["export", "notion"];
    args.extend_from_slice(extra);
    parse_csv(&ok(dir, HUMAN, &args).stdout)
}

fn shown(dir: &Path, id: &str) -> Value {
    ok(dir, HUMAN, &["show", id, "--json"]).json()
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn export_notion_writes_notion_columns_and_values() {
    let dir = with_fields("export-columns");
    let d = dir.path();
    ok(
        d,
        HUMAN,
        &[
            "add",
            "Design the toolbar",
            "--priority",
            "high",
            "--field",
            "project=dashboard",
            "--field",
            "team=design",
            "--label",
            "ux",
            "--label",
            "frontend",
            "--body",
            "Sketch first",
            "--assignee",
            "human:ana",
        ],
    );
    ok(d, HUMAN, &["add", "Ship it", "--priority", "med"]);
    ok(d, HUMAN, &["update", "2", "--block", "1"]);
    ok(d, CLAUDE, &["start", "1"]);

    let rows = preview(d, &[]);
    assert_eq!(
        rows[0],
        [
            "Name",
            "Status",
            "Priority",
            "Project",
            "Team",
            "Tags",
            "Assignee",
            "Description",
            "Blocked by",
            "hippo-task ID"
        ]
    );
    assert_eq!(
        rows.len(),
        3,
        "a header and one row per open task: {rows:?}"
    );
    let first_id = shown(d, "1")["id"].as_str().unwrap().to_string();
    assert_eq!(
        rows[1],
        [
            "Design the toolbar",
            "In progress",
            "High",
            "dashboard",
            "design",
            "frontend, ux",
            "human:ana",
            "Sketch first",
            "",
            first_id.as_str()
        ]
    );
    assert_eq!(cell(&rows, 2, "Name"), "Ship it");
    assert_eq!(cell(&rows, 2, "Status"), "Not started");
    assert_eq!(cell(&rows, 2, "Priority"), "Medium");
    assert_eq!(cell(&rows, 2, "Project"), "");
    assert_eq!(cell(&rows, 2, "Blocked by"), "#1 Design the toolbar");
}

#[test]
fn without_declared_fields_there_are_no_field_columns_and_quoting_survives() {
    let dir = TempDir::new("export-quoting");
    let d = dir.path();
    ok(
        d,
        HUMAN,
        &[
            "add",
            "Say \"hi\", then leave",
            "--body",
            "line one\nline two",
        ],
    );
    let rows = preview(d, &[]);
    assert_eq!(
        rows[0],
        [
            "Name",
            "Status",
            "Priority",
            "Tags",
            "Assignee",
            "Description",
            "Blocked by",
            "hippo-task ID"
        ]
    );
    assert_eq!(cell(&rows, 1, "Name"), "Say \"hi\", then leave");
    assert_eq!(cell(&rows, 1, "Description"), "line one\nline two");
    assert_eq!(
        cell(&rows, 1, "Priority"),
        "",
        "no priority is a blank cell"
    );
}

#[test]
fn a_preview_records_nothing_and_an_export_to_a_file_is_remembered() {
    let dir = TempDir::new("export-remember");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    ok(d, HUMAN, &["add", "b"]);

    // A preview on stdout records nothing.
    assert_eq!(preview(d, &[]).len(), 3);
    assert_eq!(shown(d, "1")["exported"], json!({}));

    // To a file: written, and remembered on each task.
    let first = d.join("first.csv");
    let run = ok(d, HUMAN, &["export", "notion", "--out", path(&first)]);
    assert!(run.stdout.contains("2 tasks"), "{}", run.stdout);
    assert_eq!(parse_csv(&fs::read_to_string(&first).unwrap()).len(), 3);
    assert!(shown(d, "1")["exported"]["notion"].as_i64().is_some());

    // The next export has only what's new…
    ok(d, HUMAN, &["add", "c"]);
    let second = d.join("second.csv");
    ok(d, HUMAN, &["export", "notion", "--out", path(&second)]);
    let rows = parse_csv(&fs::read_to_string(&second).unwrap());
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(cell(&rows, 1, "Name"), "c");

    // …unless --again asks for the ones already exported too.
    assert_eq!(preview(d, &["--again"]).len(), 4);
}

#[test]
fn an_export_never_overwrites_a_file() {
    // If the last file was never imported, overwriting it would lose tasks
    // that are already marked as exported.
    let dir = TempDir::new("export-no-overwrite");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    let file = d.join("tasks.csv");
    fs::write(&file, "not imported yet").unwrap();
    let run = tasks(d, HUMAN, &["export", "notion", "--out", path(&file)]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert_eq!(fs::read_to_string(&file).unwrap(), "not imported yet");
    assert_eq!(shown(d, "1")["exported"], json!({}), "nothing recorded");
}

#[test]
fn with_nothing_to_export_no_file_is_written() {
    let dir = TempDir::new("export-nothing");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    ok(
        d,
        HUMAN,
        &["export", "notion", "--out", path(&d.join("first.csv"))],
    );
    let second = d.join("second.csv");
    let run = ok(d, HUMAN, &["export", "notion", "--out", path(&second)]);
    assert!(!second.exists(), "no empty file");
    assert!(run.stdout.contains("nothing"), "{}", run.stdout);
}

#[test]
fn an_export_is_bookkeeping_not_a_change() {
    let dir = TempDir::new("export-bookkeeping");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    let before = shown(d, "1");
    ok(
        d,
        HUMAN,
        &["export", "notion", "--out", path(&d.join("a.csv"))],
    );
    let after = shown(d, "1");
    assert_eq!(after["seq"], before["seq"]);
    assert_eq!(after["updated_ms"], before["updated_ms"]);
    let history = ok(d, HUMAN, &["show", "1"]).stdout;
    assert!(history.contains("export"), "{history}");
    assert!(history.contains("notion"), "{history}");
}

#[test]
fn tasks_changed_since_their_export_are_named() {
    let dir = TempDir::new("export-changed");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    ok(
        d,
        HUMAN,
        &["export", "notion", "--out", path(&d.join("first.csv"))],
    );
    ok(d, HUMAN, &["update", "1", "--title", "a, revised"]);
    ok(d, HUMAN, &["add", "b"]);
    let run = ok(
        d,
        HUMAN,
        &["export", "notion", "--out", path(&d.join("second.csv"))],
    );
    assert!(run.stdout.contains("changed since"), "{}", run.stdout);
    assert!(run.stdout.contains("#1"), "{}", run.stdout);
}

/// The numbers of the tasks a real export reports as changed since their last one.
fn changed_after_export(dir: &Path, file: &str) -> Vec<u64> {
    let out = dir.join(file);
    let report = ok(
        dir,
        HUMAN,
        &["export", "notion", "--out", path(&out), "--json"],
    )
    .json();
    report["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["num"].as_u64().unwrap())
        .collect()
}

#[test]
fn a_task_closed_after_its_export_is_named_as_changed() {
    // Its Notion row still says "Not started" — the commonest change of all.
    let dir = TempDir::new("export-closed-since");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    ok(d, HUMAN, &["add", "b"]);
    assert_eq!(changed_after_export(d, "first.csv"), Vec::<u64>::new());
    ok(d, HUMAN, &["done", "1"]);
    ok(d, HUMAN, &["update", "2", "--state", "cancelled"]);
    assert_eq!(changed_after_export(d, "second.csv"), [1, 2]);
    let text = ok(d, HUMAN, &["export", "notion"]).stderr;
    assert!(text.contains("#1"), "the preview names it too: {text}");
}

#[test]
fn a_note_alone_leaves_the_exported_row_unchanged() {
    // Notes aren't in the CSV, so there's nothing to fix in Notion.
    let dir = TempDir::new("export-note-only");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    changed_after_export(d, "first.csv");
    ok(d, HUMAN, &["note", "1", "looked at it"]);
    ok(d, CLAUDE, &["start", "1"]);
    ok(d, CLAUDE, &["release", "1"]);
    assert_eq!(
        changed_after_export(d, "second.csv"),
        Vec::<u64>::new(),
        "started and released again: the row reads as it did"
    );
}

#[test]
fn a_blocker_closing_changes_the_blocked_tasks_row() {
    // `Blocked by` lists open blockers only, so the cell empties out.
    let dir = TempDir::new("export-blocker-closed");
    let d = dir.path();
    ok(d, HUMAN, &["add", "blocker"]);
    ok(d, HUMAN, &["add", "blocked"]);
    ok(d, HUMAN, &["update", "2", "--block", "1"]);
    changed_after_export(d, "first.csv");
    ok(d, HUMAN, &["done", "1"]);
    assert_eq!(changed_after_export(d, "second.csv"), [1, 2]);
}

#[test]
fn a_change_made_and_undone_since_the_export_is_no_change() {
    let dir = TempDir::new("export-undone");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    changed_after_export(d, "first.csv");
    ok(d, HUMAN, &["update", "1", "--title", "b"]);
    ok(d, HUMAN, &["update", "1", "--title", "a"]);
    assert_eq!(changed_after_export(d, "second.csv"), Vec::<u64>::new());
}

#[test]
fn closed_tasks_stay_out_unless_asked_for() {
    let dir = TempDir::new("export-closed");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    ok(d, HUMAN, &["add", "b"]);
    ok(d, HUMAN, &["done", "2"]);
    assert_eq!(preview(d, &[]).len(), 2);
    let rows = preview(d, &["--all"]);
    assert_eq!(rows.len(), 3);
    assert_eq!(cell(&rows, 2, "Status"), "Done");
}

#[test]
fn export_narrows_by_field_and_by_label() {
    let dir = with_fields("export-filters");
    let d = dir.path();
    ok(
        d,
        HUMAN,
        &["add", "a", "--field", "project=dashboard", "--label", "bug"],
    );
    ok(d, HUMAN, &["add", "b", "--field", "project=billing"]);
    ok(d, HUMAN, &["add", "c", "--label", "bug"]);
    let names = |rows: Vec<Vec<String>>| -> Vec<String> {
        rows.iter().skip(1).map(|r| r[0].clone()).collect()
    };
    assert_eq!(names(preview(d, &["--field", "project=dashboard"])), ["a"]);
    assert_eq!(names(preview(d, &["--label", "bug"])), ["a", "c"]);
}

#[test]
fn json_export_needs_a_file_and_reports_what_it_wrote() {
    let dir = TempDir::new("export-json");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a"]);
    let run = tasks(d, HUMAN, &["export", "notion", "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");

    let file = d.join("a.csv");
    let report = ok(
        d,
        HUMAN,
        &["export", "notion", "--out", path(&file), "--json"],
    )
    .json();
    assert_eq!(report["file"], path(&file));
    let exported = report["exported"].as_array().unwrap();
    assert_eq!(exported.len(), 1);
    assert!(
        exported[0]["exported"]["notion"].as_i64().is_some(),
        "{report}"
    );
}
