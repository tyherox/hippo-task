//! The agent skill (ADR-014): `hippo-task skill --to <folder>` writes the
//! guide as an Agent Skill (agentskills.io), so an agent loads a two-line
//! description at startup and the protocol only when it works on tasks.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{bare, Run, TempDir};
use std::fs;
use std::path::{Path, PathBuf};

/// Run `skill` from `cwd` with no store, no `HIPPO_DIR`, and an agent actor
/// with no node — writing the skill needs none of them.
fn skill(cwd: &Path, args: &[&str]) -> Run {
    let out = bare()
        .arg("skill")
        .args(args)
        .current_dir(cwd)
        .env("HIPPO_ACTOR", "agent:claude")
        .output()
        .unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn written(tag: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new(tag);
    let run = skill(dir.path(), &["--to", ".claude/skills", "--json"]);
    assert_eq!(run.code, 0, "{run:#?}");
    let folder = dir.path().join(".claude/skills/hippo-task");
    (dir, folder)
}

/// The frontmatter's lines, and the body after it.
fn split(skill_md: &str) -> (Vec<String>, String) {
    let rest = skill_md
        .strip_prefix("---\n")
        .unwrap_or_else(|| panic!("no frontmatter:\n{skill_md}"));
    let (front, body) = rest.split_once("\n---\n").unwrap();
    (front.lines().map(String::from).collect(), body.to_string())
}

fn value<'a>(front: &'a [String], key: &str) -> &'a str {
    front
        .iter()
        .find_map(|l| l.strip_prefix(&format!("{key}: ")))
        .unwrap_or_else(|| panic!("no {key} in {front:?}"))
}

#[test]
fn skill_writes_the_two_files_without_a_store_or_a_node() {
    let dir = TempDir::new("skill-write");
    let run = skill(dir.path(), &["--to", ".claude/skills", "--json"]);
    assert_eq!(run.code, 0, "{run:#?}");
    let report = run.json();
    let folder = fs::canonicalize(dir.path().join(".claude/skills/hippo-task")).unwrap();
    assert_eq!(
        fs::canonicalize(report["skill"].as_str().unwrap()).unwrap(),
        folder
    );
    let files: Vec<PathBuf> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| fs::canonicalize(f.as_str().unwrap()).unwrap())
        .collect();
    assert_eq!(
        files,
        [
            folder.join("SKILL.md"),
            folder.join("references").join("json.md")
        ]
    );
    assert!(
        !dir.path().join(".hippotask").exists(),
        "no store is created"
    );
}

#[test]
fn the_frontmatter_follows_the_agent_skills_spec() {
    let (_dir, folder) = written("skill-front");
    let (front, body) = split(&fs::read_to_string(folder.join("SKILL.md")).unwrap());
    // name: 1-64 lowercase letters, digits and hyphens — and the folder's name.
    assert_eq!(value(&front, "name"), "hippo-task");
    assert_eq!(folder.file_name().unwrap(), "hippo-task");
    let description = value(&front, "description").trim_matches('"');
    assert!(!description.is_empty() && description.chars().count() <= 1024);
    assert!(!description.contains('"'), "the quoted YAML stays valid");
    assert!(
        description.contains("Use when"),
        "says when to use it: {description}"
    );
    assert!(front.contains(&"metadata:".to_string()), "{front:?}");
    assert_eq!(
        value(&front, "  hippo-task-version").trim_matches('"'),
        env!("CARGO_PKG_VERSION")
    );
    assert!(
        body.lines().count() < 500,
        "the spec asks for under 500 lines"
    );
}

#[test]
fn the_skill_is_the_guide_with_the_json_shapes_in_a_reference() {
    let (_dir, folder) = written("skill-body");
    let skill_md = fs::read_to_string(folder.join("SKILL.md")).unwrap();
    for needle in [
        "The loop:",
        "list --json --ready",
        "hippo-task similar",
        "hippo-task format --json",
        "release --all",
        "| 4 | `conflict`",
        "references/json.md",
    ] {
        assert!(skill_md.contains(needle), "SKILL.md lacks {needle:?}");
    }
    assert!(
        !skill_md.contains("Task object:"),
        "the shapes live in the reference"
    );
    let reference = fs::read_to_string(folder.join("references/json.md")).unwrap();
    for needle in ["Task object", "`format --json`", "`lease`", "Stability:"] {
        assert!(reference.contains(needle), "json.md lacks {needle:?}");
    }
}

#[test]
fn neither_the_skill_nor_the_guide_teaches_the_ui_internal_api() {
    let (dir, folder) = written("skill-no-ui");
    let missing = skill(dir.path(), &[]);
    assert_eq!(missing.code, 2, "--to is required: {missing:#?}");
    let guide = bare()
        .arg("guide")
        .current_dir(dir.path())
        .output()
        .unwrap();
    let guide = String::from_utf8_lossy(&guide.stdout).into_owned();
    for text in [
        guide,
        fs::read_to_string(folder.join("SKILL.md")).unwrap(),
        fs::read_to_string(folder.join("references/json.md")).unwrap(),
    ] {
        assert!(
            !text.contains("/api/state"),
            "agents don't call the UI's API"
        );
    }
}

#[test]
fn running_it_again_updates_its_files_and_leaves_others_alone() {
    let (dir, folder) = written("skill-again");
    fs::write(folder.join("SKILL.md"), "stale").unwrap();
    fs::write(folder.join("notes.md"), "mine").unwrap();
    let run = skill(dir.path(), &["--to", ".claude/skills"]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert!(fs::read_to_string(folder.join("SKILL.md"))
        .unwrap()
        .starts_with("---\nname: hippo-task\n"));
    assert_eq!(fs::read_to_string(folder.join("notes.md")).unwrap(), "mine");
    assert!(run.stdout.contains("SKILL.md"), "{run:#?}");
}

#[test]
fn the_identity_check_is_one_a_narrow_allowlist_permits() {
    // A Claude Code session allowed only `Bash(hippo-task:*)` can't run
    // `echo "$HIPPO_ACTOR…"` or `printenv` (ADR-014 amendment).
    let (_dir, folder) = written("skill-identity");
    let skill_md = fs::read_to_string(folder.join("SKILL.md")).unwrap();
    assert!(skill_md.contains("hippo-task whoami --json"), "{skill_md}");
    assert!(
        !skill_md.contains("echo \"$HIPPO"),
        "an identity check narrow allowlists refuse"
    );
    assert!(
        !skill_md.contains("claude-win-1"),
        "a node name two agents could both copy"
    );
}
