//! Task format (ADR-012): a team's conventions in its store's `config.toml` —
//! a guide, a description template, required fields and sections. Shown by
//! `hippo-task format`; writes that miss it warn, and are never refused.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{ok, tasks, Run, TempDir, HUMAN};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const CONFIG: &str = r#"
[fields.project]
values = ["dashboard", "billing"]

[format]
guide = "Titles start with a verb."
template = """
## Why

## Done when
- [ ]
"""
required_fields = ["project"]
required_sections = ["Done when"]
"#;

fn with_config(tag: &str, text: &str) -> TempDir {
    let dir = TempDir::new(tag);
    let store = dir.path().join(".hippotask");
    fs::create_dir_all(&store).unwrap();
    fs::write(store.join("config.toml"), text).unwrap();
    dir
}

/// The `{"warning": …}` lines a `--json` run wrote to stderr.
fn warnings(run: &Run) -> Vec<String> {
    run.stderr
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| v["warning"].as_str().map(String::from))
        .collect()
}

fn add(dir: &Path, args: &[&str]) -> Run {
    let mut all = vec!["add"];
    all.extend_from_slice(args);
    all.push("--json");
    ok(dir, HUMAN, &all)
}

const FILLED: &str = "## Why\nUsers get logged out.\n\n## Done when\n- [ ] a 401 refreshes once";

// ---- `hippo-task format` ----

#[test]
fn format_shows_the_declared_conventions() {
    let dir = with_config("format-show", CONFIG);
    let shown = ok(dir.path(), HUMAN, &["format", "--json"]).json();
    assert!(shown["config"].as_str().unwrap().ends_with("config.toml"));
    assert_eq!(shown["guide"], "Titles start with a verb.");
    assert_eq!(shown["template"], "## Why\n\n## Done when\n- [ ]\n");
    assert_eq!(shown["required_fields"], json!(["project"]));
    assert_eq!(shown["required_sections"], json!(["Done when"]));
}

#[test]
fn without_a_format_every_key_is_empty_and_the_text_shows_an_example() {
    let dir = TempDir::new("format-none");
    let shown = ok(dir.path(), HUMAN, &["format", "--json"]).json();
    assert_eq!(shown["guide"], Value::Null);
    assert_eq!(shown["template"], Value::Null);
    assert_eq!(shown["required_fields"], json!([]));
    assert_eq!(shown["required_sections"], json!([]));
    let text = ok(dir.path(), HUMAN, &["format"]).stdout;
    assert!(text.contains("[format]"), "{text}");
}

#[test]
fn a_required_field_must_be_declared() {
    let dir = with_config(
        "format-undeclared",
        "[fields.project]\n[format]\nrequired_fields = [\"projcet\"]\n",
    );
    let run = tasks(dir.path(), HUMAN, &["format", "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    let message = run.json_error()["message"].as_str().unwrap().to_string();
    assert!(message.contains("projcet"), "{message}");
    assert!(message.contains("project"), "should suggest it: {message}");
}

#[test]
fn a_required_section_must_be_a_heading_in_the_template() {
    let dir = with_config(
        "format-section",
        "[format]\ntemplate = \"## Why\\n\"\nrequired_sections = [\"Done when\"]\n",
    );
    let run = tasks(dir.path(), HUMAN, &["format", "--json"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.json_error()["message"]
        .as_str()
        .unwrap()
        .contains("Done when"));
}

#[test]
fn an_unknown_format_setting_is_warned_about_not_refused() {
    let dir = with_config(
        "format-unknown",
        "[format]\nrequried_sections = [\"Why\"]\n",
    );
    let run = ok(dir.path(), HUMAN, &["format", "--json"]);
    let w = warnings(&run).join("\n");
    assert!(w.contains("requried_sections"), "{run:#?}");
    assert!(
        w.contains("required_sections"),
        "should suggest it: {run:#?}"
    );
}

#[test]
fn format_is_a_setting_this_version_knows() {
    let dir = with_config("format-known", CONFIG);
    let run = ok(dir.path(), HUMAN, &["fields", "--json"]);
    assert!(warnings(&run).is_empty(), "{run:#?}");
}

// ---- writes warn about gaps, and still happen ----

#[test]
fn add_warns_about_a_missing_field_and_section_but_creates_the_task() {
    let dir = with_config("format-add-gaps", CONFIG);
    let run = add(dir.path(), &["Fix token refresh"]);
    assert_eq!(run.json()["num"], 1);
    let w = warnings(&run);
    assert_eq!(w.len(), 2, "one per gap: {w:?}");
    assert!(
        w.iter()
            .any(|m| m.contains("project") && m.contains("--field project=")),
        "{w:?}"
    );
    assert!(
        w.iter()
            .any(|m| m.contains("Done when") && m.contains("hippo-task desc 1")),
        "{w:?}"
    );
}

#[test]
fn a_task_that_meets_the_format_gets_no_warning() {
    let dir = with_config("format-add-ok", CONFIG);
    let run = add(
        dir.path(),
        &[
            "Fix token refresh",
            "--field",
            "project=dashboard",
            "--body",
            FILLED,
        ],
    );
    assert!(warnings(&run).is_empty(), "{run:#?}");
}

#[test]
fn an_untouched_template_still_misses_its_sections() {
    let dir = with_config("format-template", CONFIG);
    let template = "## Why\n\n## Done when\n- [ ]\n<!-- what proves it works -->\n";
    let run = add(
        dir.path(),
        &[
            "Fix token refresh",
            "--field",
            "project=dashboard",
            "--body",
            template,
        ],
    );
    let w = warnings(&run);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("Done when"), "{w:?}");
}

#[test]
fn a_section_matches_any_heading_level_case_and_trailing_colon() {
    let dir = with_config("format-heading", CONFIG);
    let body = "### done WHEN:\nThe retry happens once.";
    let run = add(
        dir.path(),
        &[
            "Fix token refresh",
            "--field",
            "project=dashboard",
            "--body",
            body,
        ],
    );
    assert!(warnings(&run).is_empty(), "{run:#?}");
}

#[test]
fn text_under_a_subheading_fills_its_section_but_the_next_section_does_not() {
    let dir = with_config("format-nesting", CONFIG);
    let nested = "## Done when\n### Checks\nA 401 refreshes once.";
    let run = add(
        dir.path(),
        &["A", "--field", "project=dashboard", "--body", nested],
    );
    assert!(warnings(&run).is_empty(), "{run:#?}");
    let next = "## Done when\n## Notes\nSomething else entirely.";
    let run = add(
        dir.path(),
        &["B", "--field", "project=dashboard", "--body", next],
    );
    assert_eq!(warnings(&run).len(), 1, "{run:#?}");
}

#[test]
fn desc_and_field_updates_check_the_format_again() {
    let dir = with_config("format-update", CONFIG);
    add(dir.path(), &["Fix token refresh"]);
    let run = ok(dir.path(), HUMAN, &["desc", "1", FILLED, "--json"]);
    let w = warnings(&run);
    assert_eq!(w.len(), 1, "only the field is missing now: {w:?}");
    assert!(w[0].contains("project"), "{w:?}");
    let run = ok(
        dir.path(),
        HUMAN,
        &["update", "1", "--field", "project=billing", "--json"],
    );
    assert!(warnings(&run).is_empty(), "{run:#?}");
}

#[test]
fn other_updates_and_closed_tasks_do_not_warn() {
    let dir = with_config("format-quiet", CONFIG);
    add(dir.path(), &["Fix token refresh"]);
    // A PM triaging priority isn't shaping the task: no nagging.
    let run = ok(
        dir.path(),
        HUMAN,
        &["update", "1", "--priority", "high", "--json"],
    );
    assert!(warnings(&run).is_empty(), "{run:#?}");
    ok(dir.path(), HUMAN, &["done", "1"]);
    let run = ok(dir.path(), HUMAN, &["desc", "1", "Shipped.", "--json"]);
    assert!(warnings(&run).is_empty(), "{run:#?}");
}

#[test]
fn without_a_format_add_stays_quiet() {
    let dir = TempDir::new("format-absent");
    let run = add(dir.path(), &["Anything"]);
    assert!(warnings(&run).is_empty(), "{run:#?}");
}

#[test]
fn a_broken_config_never_fails_an_add_that_was_written() {
    let dir = with_config("format-broken", "[format\n");
    let run = add(dir.path(), &["Still recorded"]);
    assert_eq!(run.json()["title"], "Still recorded");
    assert!(
        warnings(&run).iter().any(|w| w.contains("config.toml")),
        "{run:#?}"
    );
}
