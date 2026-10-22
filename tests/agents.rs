//! `init` sets up the project's agents (ADR-015): the skill, session hooks
//! merged into each agent's own settings, and one line in the project's
//! instructions — so nobody edits AGENTS.md or settings.json by hand.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Run, TempDir};
use hippo_task::agents::{self, Agent};
use std::fs;
use std::io::Cursor;
use std::path::Path;

const HUMAN: (&str, &str) = ("human:tester", "n0");
const LINE: &str =
    "Tasks are coordinated with hippo-task: use its skill, or run `hippo-task guide` if you have none.";

/// Run the real binary from `cwd`, with no `HIPPO_DIR`.
fn at(cwd: &Path, args: &[&str]) -> Run {
    let out = common::bare()
        .args(args)
        .current_dir(cwd)
        .env("HIPPO_ACTOR", HUMAN.0)
        .env("HIPPO_NODE", HUMAN.1)
        .output()
        .expect("run the hippo-task binary");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn ok_at(cwd: &Path, args: &[&str]) -> Run {
    let run = at(cwd, args);
    assert_eq!(run.code, 0, "expected success for {args:?}:\n{run:#?}");
    run
}

fn repo(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    fs::create_dir(dir.path().join(".git")).unwrap();
    dir
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn json_file(path: &Path) -> serde_json::Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every hook command a settings file runs for `event`, in order.
fn commands(settings: &serde_json::Value, event: &str) -> Vec<String> {
    settings["hooks"][event]
        .as_array()
        .unwrap_or_else(|| panic!("no {event} hooks in {settings}"))
        .iter()
        .flat_map(|group| group["hooks"].as_array().unwrap().iter())
        .map(|hook| hook["command"].as_str().unwrap().to_string())
        .collect()
}

// ---- setting up ----

#[test]
fn init_sets_up_claude_code_and_codex_when_asked() {
    let dir = repo("agents-both");
    let report = ok_at(
        dir.path(),
        &["init", "--here", "--agents", "claude,codex", "--json"],
    )
    .json();

    let skills = [".claude/skills/hippo-task", ".agents/skills/hippo-task"];
    for skill in skills {
        assert!(
            dir.path().join(skill).join("SKILL.md").is_file(),
            "{skill}: {report}"
        );
    }
    let claude = json_file(&dir.path().join(".claude/settings.json"));
    assert_eq!(
        commands(&claude, "SessionStart"),
        ["hippo-task hook session-start"]
    );
    assert_eq!(
        commands(&claude, "SessionEnd"),
        ["hippo-task hook session-end"]
    );
    let codex = json_file(&dir.path().join(".codex/hooks.json"));
    assert_eq!(
        commands(&codex, "SessionStart"),
        ["hippo-task hook session-start --agent codex"]
    );
    assert_eq!(
        commands(&codex, "SessionEnd"),
        ["hippo-task hook session-end --agent codex"]
    );
    assert_eq!(
        codex["hooks"]["SessionEnd"][0]["hooks"][0]["timeout"], 3,
        "Codex gives an end hook 1 second unless it asks for more"
    );
    assert_eq!(read(&dir.path().join("AGENTS.md")), format!("{LINE}\n"));

    // The report names every file, so a script (or a person) knows what changed.
    let listed: Vec<&str> = report["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["agent"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ["claude", "codex"]);
    for (entry, (skill, hooks)) in report["agents"].as_array().unwrap().iter().zip([
        (skills[0], ".claude/settings.json"),
        (skills[1], ".codex/hooks.json"),
    ]) {
        // Path::ends_with compares components, so this holds on Windows too.
        assert!(
            Path::new(entry["skill"].as_str().unwrap()).ends_with(skill),
            "{entry}"
        );
        assert!(
            Path::new(entry["hooks"].as_str().unwrap()).ends_with(hooks),
            "{entry}"
        );
        assert_eq!(entry["hooks_added"], true, "{entry}");
    }
    assert!(Path::new(report["instructions"]["file"].as_str().unwrap()).ends_with("AGENTS.md"));
    assert_eq!(report["instructions"]["added"], true);
}

#[test]
fn init_without_agents_touches_no_agent_files() {
    let dir = repo("agents-none");
    let report = ok_at(dir.path(), &["init", "--here", "--json"]).json();
    assert_eq!(report["agents"], serde_json::json!([]));
    assert!(report["instructions"].is_null(), "{report}");
    for path in [".claude", ".agents", ".codex", "AGENTS.md", "CLAUDE.md"] {
        assert!(!dir.path().join(path).exists(), "{path} was created");
    }
}

#[test]
fn init_on_a_set_up_project_needs_no_place_to_set_up_agents() {
    let dir = repo("agents-later");
    ok_at(dir.path(), &["init", "--here"]);
    // No --here: the store already exists, and init never moves one.
    let report = ok_at(dir.path(), &["init", "--agents", "codex", "--json"]).json();
    assert_eq!(report["created"], false);
    assert!(dir
        .path()
        .join(".agents/skills/hippo-task/SKILL.md")
        .is_file());
    assert!(
        !dir.path().join(".claude").exists(),
        "only Codex was asked for"
    );
}

#[test]
fn init_says_what_it_set_up_and_that_codex_hooks_need_trusting() {
    let dir = repo("agents-text");
    let run = ok_at(dir.path(), &["init", "--here", "--agents", "claude,codex"]);
    for needle in [
        ".claude/skills/hippo-task",
        ".claude/settings.json",
        ".agents/skills/hippo-task",
        ".codex/hooks.json",
        "AGENTS.md",
        "/hooks",
    ] {
        assert!(
            run.stdout.contains(needle),
            "init lacks {needle:?}:\n{}",
            run.stdout
        );
    }
}

#[test]
fn init_without_agents_says_how_to_set_them_up() {
    let dir = repo("agents-hint");
    let run = ok_at(dir.path(), &["init", "--here"]);
    assert!(
        run.stdout.contains("hippo-task init --agents claude,codex"),
        "{}",
        run.stdout
    );
}

// ---- merging into what's already there ----

/// A settings file the way a person (or Claude Code) left it: keys in no
/// particular order, a hook of their own, formatting of their own.
const SETTINGS: &str = r#"{
  "permissions": {
    "deny": [],
    "allow": [
      "Bash(make verify)",

      "Bash(ls)"
    ]
  },
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "hooks": {
    "PreToolUse": [ { "matcher": "Bash", "hooks": [ { "type": "command", "command": "./audit.sh" } ] } ],
    "SessionStart": [
      { "hooks": [ { "type": "command", "command": "./warm-cache.sh" } ] }
    ]
  },
  "env": { "B": "2", "A": "1" }
}
"#;

#[test]
fn hooks_are_added_next_to_everything_already_in_the_settings() {
    let dir = repo("agents-merge");
    fs::create_dir(dir.path().join(".claude")).unwrap();
    let file = dir.path().join(".claude/settings.json");
    fs::write(&file, SETTINGS).unwrap();
    ok_at(dir.path(), &["init", "--here", "--agents", "claude"]);

    let text = read(&file);
    let settings: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        commands(&settings, "SessionStart"),
        ["./warm-cache.sh", "hippo-task hook session-start"],
        "the person's own hook stays, and runs first"
    );
    assert_eq!(
        commands(&settings, "SessionEnd"),
        ["hippo-task hook session-end"]
    );
    assert_eq!(
        settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "./audit.sh"
    );
    // Untouched values come back exactly as written, keys in file order.
    for kept in [
        "\"permissions\": {\n    \"deny\": [],\n    \"allow\": [\n      \"Bash(make verify)\",\n\n      \"Bash(ls)\"\n    ]\n  }",
        "\"env\": { \"B\": \"2\", \"A\": \"1\" }",
        "{ \"matcher\": \"Bash\", \"hooks\": [ { \"type\": \"command\", \"command\": \"./audit.sh\" } ] }",
    ] {
        assert!(text.contains(kept), "lost {kept:?} in:\n{text}");
    }
    let pos = |key: &str| text.find(&format!("\"{key}\"")).unwrap();
    assert!(
        pos("permissions") < pos("$schema")
            && pos("$schema") < pos("hooks")
            && pos("hooks") < pos("env"),
        "keys stay in file order:\n{text}"
    );
}

#[test]
fn running_init_again_changes_nothing() {
    let dir = repo("agents-again");
    fs::write(dir.path().join("AGENTS.md"), "# Agents\n\nBe kind.\n").unwrap();
    let args = ["init", "--here", "--agents", "claude,codex", "--json"];
    ok_at(dir.path(), &args);
    let files = [
        ".claude/settings.json",
        ".codex/hooks.json",
        "AGENTS.md",
        ".claude/skills/hippo-task/SKILL.md",
    ];
    let before: Vec<String> = files.iter().map(|f| read(&dir.path().join(f))).collect();
    let again = ok_at(dir.path(), &args).json();
    let after: Vec<String> = files.iter().map(|f| read(&dir.path().join(f))).collect();
    assert_eq!(before, after);
    for entry in again["agents"].as_array().unwrap() {
        assert_eq!(entry["hooks_added"], false, "{entry}");
    }
    assert_eq!(again["instructions"]["added"], false);
    assert_eq!(
        read(&dir.path().join("AGENTS.md")),
        format!("# Agents\n\nBe kind.\n\n{LINE}\n")
    );
}

#[test]
fn a_hook_the_person_already_added_is_not_added_twice() {
    let dir = repo("agents-own-path");
    fs::create_dir(dir.path().join(".claude")).unwrap();
    let file = dir.path().join(".claude/settings.json");
    // Their own copy, at a full path, with a matcher.
    let own = r#"{"hooks": {"SessionStart": [{"matcher": "startup", "hooks": [{"type": "command", "command": "/opt/bin/hippo-task hook session-start"}]}]}}"#;
    fs::write(&file, own).unwrap();
    ok_at(dir.path(), &["init", "--here", "--agents", "claude"]);
    let settings = json_file(&file);
    assert_eq!(
        commands(&settings, "SessionStart"),
        ["/opt/bin/hippo-task hook session-start"]
    );
    assert_eq!(
        commands(&settings, "SessionEnd"),
        ["hippo-task hook session-end"]
    );
}

#[test]
fn a_settings_file_init_cannot_read_is_left_alone_with_a_warning() {
    for (tag, broken) in [
        ("agents-broken-text", "{ \"permissions\": "),
        ("agents-broken-array", "[]"),
        ("agents-broken-hooks", r#"{"hooks": []}"#),
        ("agents-broken-event", r#"{"hooks": {"SessionEnd": {}}}"#),
    ] {
        let dir = repo(tag);
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let file = dir.path().join(".claude/settings.json");
        fs::write(&file, broken).unwrap();
        let run = ok_at(
            dir.path(),
            &["init", "--here", "--agents", "claude", "--json"],
        );
        assert_eq!(read(&file), broken, "{tag}: the file was changed");
        let report = run.json();
        assert_eq!(report["agents"][0]["hooks_added"], false, "{tag}");
        let warning = run
            .stderr
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .find_map(|l| l["warning"].as_str().map(String::from))
            .unwrap_or_else(|| panic!("{tag}: no warning: {run:#?}"));
        assert!(
            warning.contains("settings.json") && warning.contains("hippo-task hook session-start"),
            "{tag}: the warning says which file, and what to add: {warning}"
        );
        // Everything else still happened.
        assert!(dir.path().join(".hippotask").is_dir(), "{tag}");
        assert!(
            dir.path()
                .join(".claude/skills/hippo-task/SKILL.md")
                .is_file(),
            "{tag}"
        );
    }
}

// ---- the instructions line ----

#[test]
fn the_line_goes_into_the_instructions_file_the_project_already_has() {
    // Only CLAUDE.md: the line goes there, and no AGENTS.md appears.
    let dir = repo("agents-claude-md");
    fs::write(dir.path().join("CLAUDE.md"), "No trailing newline").unwrap();
    ok_at(dir.path(), &["init", "--here", "--agents", "codex"]);
    assert_eq!(
        read(&dir.path().join("CLAUDE.md")),
        format!("No trailing newline\n\n{LINE}\n")
    );
    assert!(!dir.path().join("AGENTS.md").exists());

    // Both: AGENTS.md wins.
    let dir = repo("agents-both-md");
    fs::write(dir.path().join("AGENTS.md"), "a\n").unwrap();
    fs::write(dir.path().join("CLAUDE.md"), "c\n").unwrap();
    ok_at(dir.path(), &["init", "--here", "--agents", "claude"]);
    assert_eq!(
        read(&dir.path().join("AGENTS.md")),
        format!("a\n\n{LINE}\n")
    );
    assert_eq!(read(&dir.path().join("CLAUDE.md")), "c\n");
}

#[test]
fn with_no_instructions_file_claude_code_alone_gets_claude_md() {
    let dir = repo("agents-new-claude-md");
    ok_at(dir.path(), &["init", "--here", "--agents", "claude"]);
    assert_eq!(read(&dir.path().join("CLAUDE.md")), format!("{LINE}\n"));
    assert!(!dir.path().join("AGENTS.md").exists());
}

#[test]
fn a_file_that_already_mentions_hippo_task_is_left_alone() {
    let dir = repo("agents-mentioned");
    let own = "Use hippo-task for the backlog.\n";
    fs::write(dir.path().join("AGENTS.md"), own).unwrap();
    let report = ok_at(
        dir.path(),
        &["init", "--here", "--agents", "codex", "--json"],
    )
    .json();
    assert_eq!(read(&dir.path().join("AGENTS.md")), own);
    assert_eq!(report["instructions"]["added"], false);
}

// ---- the question (library level, so no terminal is needed) ----

fn ask(answer: &str) -> Vec<Agent> {
    let mut out = Vec::new();
    agents::ask(&mut Cursor::new(answer), &mut out).unwrap()
}

#[test]
fn the_question_defaults_to_both_agents() {
    assert_eq!(ask("\n"), [Agent::Claude, Agent::Codex]);
    assert_eq!(ask(""), [Agent::Claude, Agent::Codex], "end of input");
    assert_eq!(ask("1\n"), [Agent::Claude, Agent::Codex]);
    assert_eq!(ask("2\n"), [Agent::Claude]);
    assert_eq!(ask("3\n"), [Agent::Codex]);
    assert_eq!(ask("4\n"), []);
    let mut out = Vec::new();
    assert!(agents::ask(&mut Cursor::new("5\n"), &mut out).is_err());
}
