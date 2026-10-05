//! The docs must match the binary: "no stale self-references", enforced as a
//! test, so CI fails on drift instead
//! of a reader finding it weeks later. (schema-design.md went stale exactly
//! this way before 0.1.0.)
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{ok, TempDir, HUMAN};
use hippo_task::error::Error;

const README: &str = include_str!("../README.md");
const AGENTS: &str = include_str!("../AGENTS.md");
const CHANGELOG: &str = include_str!("../CHANGELOG.md");

/// Subcommand names, read from `hippo-task --help` (the binary is the source of truth).
fn subcommands() -> Vec<String> {
    let out = common::bare().arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&out.stdout).into_owned();
    help.lines()
        .skip_while(|l| !l.starts_with("Commands:"))
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(String::from)
        .collect()
}

/// Every `hippo-task <word>` inside code — fenced blocks and inline `code` spans.
fn commands_mentioned(doc: &str) -> Vec<String> {
    let mut code = String::new();
    let mut in_fence = false;
    for line in doc.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        } else if in_fence {
            code.push_str(line);
            code.push('\n');
        } else {
            for span in line.split('`').skip(1).step_by(2) {
                code.push_str(span);
                code.push('\n');
            }
        }
    }
    let mut found = Vec::new();
    for line in code.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        for pair in words.windows(2) {
            let next = pair[1];
            if pair[0] == "hippo-task" && next.chars().all(|c| c.is_ascii_lowercase()) {
                found.push(next.to_string());
            }
        }
    }
    found
}

#[test]
fn every_command_is_documented_in_the_readme() {
    let mentioned = commands_mentioned(README);
    let commands = subcommands();
    assert!(commands.len() >= 10, "couldn't parse --help: {commands:?}");
    for command in commands {
        assert!(
            mentioned.contains(&command),
            "README.md never shows `hippo-task {command}`"
        );
    }
}

#[test]
fn docs_only_mention_commands_that_exist() {
    let commands = subcommands();
    for (name, doc) in [("README.md", README), ("AGENTS.md", AGENTS)] {
        for word in commands_mentioned(doc) {
            assert!(
                commands.contains(&word),
                "{name} mentions `hippo-task {word}`, which doesn't exist"
            );
        }
    }
}

#[test]
fn exit_codes_are_documented_exactly() {
    let every_error = [
        Error::io("", std::io::Error::other("x")),
        Error::Usage(String::new()),
        Error::NotFound(String::new()),
        Error::Conflict(String::new()),
        Error::Stale(String::new()),
    ];
    for e in &every_error {
        let row = format!("| {} | `{}`", e.exit_code(), e.kind());
        assert!(
            README.contains(&row),
            "README.md exit-code table lacks {row:?}"
        );
        assert!(
            AGENTS.contains(&row),
            "AGENTS.md exit-code table lacks {row:?}"
        );
    }
}

#[test]
fn every_json_field_is_documented_for_agents() {
    let dir = TempDir::new("docs-json");
    ok(dir.path(), HUMAN, &["add", "x", "--label", "l"]);
    ok(dir.path(), HUMAN, &["start", "1"]);
    let shown = ok(dir.path(), HUMAN, &["show", "1", "--json"]).json();
    let released = ok(dir.path(), HUMAN, &["release", "1", "--json"]).json();

    let mut fields: Vec<String> = shown.as_object().unwrap().keys().cloned().collect();
    fields.extend(shown["lease"].as_object().unwrap().keys().cloned());
    fields.extend(shown["events"][0].as_object().unwrap().keys().cloned());
    fields.extend(released.as_object().unwrap().keys().cloned());
    for field in fields {
        assert!(
            AGENTS.contains(&format!("`{field}`")),
            "AGENTS.md doesn't document the JSON field `{field}`"
        );
    }
}

#[test]
fn the_changelog_covers_this_version() {
    let heading = format!("## [{}]", env!("CARGO_PKG_VERSION"));
    assert!(
        CHANGELOG.contains(&heading),
        "CHANGELOG.md has no {heading} entry"
    );
}
