//! Similar tasks (ADR-013): `hippo-task similar` ranks tasks by the distinctive
//! words they share with a text, so an agent can read the likely originals
//! before filing — and `add` warns when an open task has the same title.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{ok, tasks, TempDir, HUMAN};
use std::path::Path;

/// The task numbers `similar` returns, best first.
fn similar(dir: &Path, args: &[&str]) -> Vec<u64> {
    let mut all = vec!["similar", "--json"];
    all.extend_from_slice(args);
    ok(dir, HUMAN, &all)
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["num"].as_u64().unwrap())
        .collect()
}

fn add(dir: &Path, title: &str) {
    ok(dir, HUMAN, &["add", title]);
}

/// A small backlog where common words ("fix", "score") appear everywhere and
/// the identifiers appear once — like the pilot's 36 fix tickets.
fn backlog(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    for title in [
        "Fix session_token keyed by provider label", // 1
        "Fix the total for missed payments",         // 2
        "Owner badges mark shared owners as wrong",  // 3
        "Fix settings.yaml: no loader or validator", // 4
        "Fix the check step's detector list",        // 5
        "Fix: draw the settings panel",              // 6
        "Fix total rounding in the summary",         // 7
    ] {
        add(dir.path(), title);
    }
    dir
}

#[test]
fn a_restated_task_ranks_its_original_first() {
    let dir = backlog("similar-rank");
    // The pilot's duplicates were restatements, not copies.
    let found = similar(
        dir.path(),
        &["session_token breaks when the provider relabels a merge"],
    );
    assert_eq!(found.first(), Some(&1), "got {found:?}");
    let found = similar(dir.path(), &["Owner badge hides shared projects"]);
    assert_eq!(found.first(), Some(&3), "got {found:?}");
}

#[test]
fn rare_words_outweigh_common_ones() {
    let dir = backlog("similar-rare");
    // "fix" and "total" are on most tasks; "missed" is on one.
    let found = similar(dir.path(), &["Fix total: missed payments never change"]);
    assert_eq!(found.first(), Some(&2), "got {found:?}");
}

#[test]
fn identifiers_stay_whole() {
    let dir = backlog("similar-ident");
    let found = similar(dir.path(), &["settings.yaml loader"]);
    assert_eq!(found.first(), Some(&4), "got {found:?}");
    assert!(
        !found.contains(&6),
        "`settings.yaml` must not match plain `settings`: {found:?}"
    );
}

#[test]
fn it_returns_at_most_five_by_default_and_limit_changes_that() {
    let dir = backlog("similar-limit");
    // Six tasks mention "fix".
    assert_eq!(similar(dir.path(), &["fix"]).len(), 5);
    assert_eq!(similar(dir.path(), &["fix", "--limit", "2"]).len(), 2);
    let zero = tasks(
        dir.path(),
        HUMAN,
        &["similar", "fix", "--limit", "0", "--json"],
    );
    assert_eq!(zero.code, 2, "{zero:#?}");
}

#[test]
fn nothing_in_common_is_an_empty_list_not_an_error() {
    let dir = backlog("similar-none");
    assert!(similar(dir.path(), &["quarterly invoice export"]).is_empty());
}

#[test]
fn empty_text_is_refused() {
    let dir = backlog("similar-empty");
    let run = tasks(dir.path(), HUMAN, &["similar", "  ", "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert_eq!(run.json_error()["error"], "usage");
}

#[test]
fn it_searches_every_state_since_the_original_may_be_done() {
    let dir = backlog("similar-done");
    ok(dir.path(), HUMAN, &["done", "1"]);
    assert_eq!(
        similar(dir.path(), &["session_token label"]).first(),
        Some(&1)
    );
}

#[test]
fn tasks_marked_duplicate_are_skipped_so_the_original_surfaces() {
    let dir = backlog("similar-dup");
    add(dir.path(), "session_token keyed by provider label again"); // 8
    ok(dir.path(), HUMAN, &["update", "8", "--duplicate-of", "1"]);
    let found = similar(dir.path(), &["session_token provider label"]);
    assert_eq!(found.first(), Some(&1), "got {found:?}");
    assert!(
        !found.contains(&8),
        "a marked duplicate came back: {found:?}"
    );
}

#[test]
fn descriptions_and_captions_count_but_media_paths_do_not() {
    let dir = TempDir::new("similar-body");
    add(dir.path(), "Unrelated title");
    ok(
        dir.path(),
        HUMAN,
        &[
            "add",
            "Another title",
            "--body",
            "The tokenizer drops emoji.",
        ],
    );
    assert_eq!(similar(dir.path(), &["tokenizer emoji"]), vec![2]);
    // A store image link: its caption is text, its path isn't.
    let image = dir.path().join("shot.png");
    std::fs::write(&image, PNG).unwrap();
    let body = format!("![login screen]({})", image.display());
    ok(dir.path(), HUMAN, &["add", "Third", "--body", &body]);
    assert_eq!(similar(dir.path(), &["login screen"]), vec![3]);
    assert!(similar(dir.path(), &["media"]).is_empty());
}

#[test]
fn json_is_an_array_of_task_objects_like_list() {
    let dir = backlog("similar-shape");
    let found = ok(dir.path(), HUMAN, &["similar", "session_token", "--json"]).json();
    let listed = ok(dir.path(), HUMAN, &["list", "--json"]).json();
    let keys = |v: &serde_json::Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(keys(&found[0]), keys(&listed[0]));
}

#[test]
fn text_output_lists_one_task_per_line() {
    let dir = backlog("similar-text");
    let run = ok(dir.path(), HUMAN, &["similar", "session_token"]);
    let first = run.stdout.lines().next().unwrap();
    assert!(first.starts_with("#1 "), "{run:#?}");
    assert!(first.contains("session_token"), "{run:#?}");
}

// ---- add warns about an identical open title ----

#[test]
fn add_warns_when_an_open_task_has_the_same_title() {
    let dir = TempDir::new("similar-same-title");
    add(dir.path(), "Fix the token refresh bug!");
    let run = ok(
        dir.path(),
        HUMAN,
        &["add", "fix the Token-Refresh  bug", "--json"],
    );
    assert_eq!(run.json()["num"], 2, "the task is still created: {run:#?}");
    let warning = run
        .stderr
        .lines()
        .find(|l| l.contains("\"warning\""))
        .unwrap_or_else(|| panic!("no warning: {run:#?}"));
    let warning: serde_json::Value = serde_json::from_str(warning).unwrap();
    let text = warning["warning"].as_str().unwrap();
    assert!(text.contains("#1"), "{text}");
    assert!(text.contains("note"), "should suggest a note there: {text}");
}

#[test]
fn add_does_not_warn_about_closed_tasks_or_different_titles() {
    let dir = TempDir::new("similar-same-closed");
    add(dir.path(), "Weekly release notes");
    ok(dir.path(), HUMAN, &["done", "1"]);
    let run = ok(
        dir.path(),
        HUMAN,
        &["add", "Weekly release notes", "--json"],
    );
    assert!(!run.stderr.contains("warning"), "{run:#?}");
    let run = ok(
        dir.path(),
        HUMAN,
        &["add", "Weekly release notes, part 2", "--json"],
    );
    assert!(!run.stderr.contains("warning"), "{run:#?}");
}

/// Enough of a PNG for the store's signature check (as in tests/media.rs).
const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
