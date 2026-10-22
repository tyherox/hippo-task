//! Session hooks for Claude Code (ADR-014) and Codex (ADR-015): `hook
//! session-start` gives each session its own identity (through
//! `$CLAUDE_ENV_FILE`, or for Codex one line of context), and `hook
//! session-end` gives back what that session holds. Both read the hook's JSON
//! on stdin.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{bare, ok, Run, TempDir};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

const SESSION: &str = r#"{"session_id":"1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d","hook_event_name":"SessionStart","source":"startup"}"#;
const NODE: &str = "claude-1a2b3c4d";

/// Run `hippo-task hook <event>` from `cwd` with `stdin`, and only the
/// environment given: no HIPPO_* settings unless `env` names them.
fn hook(cwd: &Path, event: &str, stdin: &str, env: &[(&str, &str)]) -> Run {
    hook_args(cwd, &[event], stdin, env)
}

/// The same, as Codex runs it: `hippo-task hook <event> --agent codex`.
fn codex_hook(cwd: &Path, event: &str, stdin: &str, env: &[(&str, &str)]) -> Run {
    hook_args(cwd, &[event, "--agent", "codex"], stdin, env)
}

fn hook_args(cwd: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Run {
    let mut cmd = bare();
    cmd.arg("hook")
        .args(args)
        .current_dir(cwd)
        .env_remove("CLAUDE_ENV_FILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// A project with a store and two tasks.
fn project(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    fs::create_dir(dir.path().join(".git")).unwrap();
    ok(dir.path(), ("human:tester", "n0"), &["add", "One"]);
    ok(dir.path(), ("human:tester", "n0"), &["add", "Two"]);
    dir
}

fn holder(dir: &Path, id: &str) -> Option<String> {
    let t = ok(dir, ("human:tester", "n0"), &["show", id, "--json"]).json();
    t["lease"].as_object().map(|l| {
        format!(
            "{}@{}",
            l["holder"].as_str().unwrap(),
            l["node"].as_str().unwrap()
        )
    })
}

// ---- session-start ----

#[test]
fn session_start_exports_an_identity_for_this_session() {
    let dir = TempDir::new("hook-start");
    let env_file = dir.path().join("claude.env");
    let run = hook(
        dir.path(),
        "session-start",
        SESSION,
        &[("CLAUDE_ENV_FILE", env_file.to_str().unwrap())],
    );
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(run.stdout, "", "nothing is added to the agent's context");
    let exports = fs::read_to_string(&env_file).unwrap();
    assert!(
        exports.contains("export HIPPO_ACTOR='agent:claude'"),
        "{exports}"
    );
    assert!(
        exports.contains(&format!("export HIPPO_NODE='{NODE}'")),
        "{exports}"
    );
}

#[test]
fn session_start_keeps_an_identity_the_person_launched_with() {
    let dir = TempDir::new("hook-start-keep");
    let env_file = dir.path().join("claude.env");
    fs::write(&env_file, "export OTHER=1\n").unwrap();
    let file = env_file.to_str().unwrap();
    let run = hook(
        dir.path(),
        "session-start",
        SESSION,
        &[
            ("CLAUDE_ENV_FILE", file),
            ("HIPPO_ACTOR", "agent:codex"),
            ("HIPPO_NODE", "win-1"),
        ],
    );
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(fs::read_to_string(&env_file).unwrap(), "export OTHER=1\n");
    // Only the missing half is filled in — and the file is appended to.
    let run = hook(
        dir.path(),
        "session-start",
        SESSION,
        &[("CLAUDE_ENV_FILE", file), ("HIPPO_ACTOR", "agent:codex")],
    );
    assert_eq!(run.code, 0, "{run:#?}");
    let exports = fs::read_to_string(&env_file).unwrap();
    assert!(exports.starts_with("export OTHER=1\n"), "{exports}");
    assert!(!exports.contains("HIPPO_ACTOR"), "{exports}");
    assert!(
        exports.contains(&format!("export HIPPO_NODE='{NODE}'")),
        "{exports}"
    );
}

#[test]
fn session_start_says_what_is_wrong_outside_a_claude_code_hook() {
    let dir = TempDir::new("hook-start-bad");
    let run = hook(dir.path(), "session-start", SESSION, &[]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.stderr.contains("CLAUDE_ENV_FILE"), "{run:#?}");
    let env_file = dir.path().join("claude.env");
    let file = env_file.to_str().unwrap();
    for stdin in ["", "not json", r#"{"session_id":"!!"}"#, r#"{"cwd":"/"}"#] {
        let run = hook(
            dir.path(),
            "session-start",
            stdin,
            &[("CLAUDE_ENV_FILE", file)],
        );
        assert_eq!(run.code, 2, "stdin {stdin:?}: {run:#?}");
        assert!(run.stderr.contains("session_id"), "{run:#?}");
    }
    assert!(!env_file.exists(), "nothing written on a bad input");
}

// ---- session-end ----

#[test]
fn session_end_gives_back_what_this_session_holds_and_nothing_else() {
    let dir = project("hook-end");
    ok(dir.path(), ("agent:claude", NODE), &["start", "1"]);
    ok(
        dir.path(),
        ("agent:claude", "claude-99999999"),
        &["start", "2"],
    );
    let run = hook(dir.path(), "session-end", SESSION, &[]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(
        holder(dir.path(), "1"),
        None,
        "this session's claim went back"
    );
    assert_eq!(
        holder(dir.path(), "2").as_deref(),
        Some("agent:claude@claude-99999999"),
        "another session's claim stays"
    );
}

#[test]
fn session_end_uses_the_identity_the_person_launched_with() {
    let dir = project("hook-end-launched");
    ok(dir.path(), ("agent:claude", "win-1"), &["start", "1"]);
    let run = hook(
        dir.path(),
        "session-end",
        SESSION,
        &[("HIPPO_ACTOR", "agent:claude"), ("HIPPO_NODE", "win-1")],
    );
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(holder(dir.path(), "1"), None);
}

#[test]
fn session_end_finds_the_store_from_the_sessions_folder() {
    let dir = project("hook-end-cwd");
    ok(dir.path(), ("agent:claude", NODE), &["start", "1"]);
    let elsewhere = TempDir::new("hook-end-elsewhere");
    let input = format!(
        r#"{{"session_id":"1a2b3c4d-0000","cwd":{}}}"#,
        serde_json::to_string(dir.path().to_str().unwrap()).unwrap()
    );
    let run = hook(elsewhere.path(), "session-end", &input, &[]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(holder(dir.path(), "1"), None);
}

#[test]
fn session_end_in_a_project_without_tasks_does_nothing_quietly() {
    let dir = TempDir::new("hook-end-none");
    fs::create_dir(dir.path().join(".git")).unwrap();
    let run = hook(dir.path(), "session-end", SESSION, &[]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(run.stderr, "", "{run:#?}");
    assert!(
        !dir.path().join(".hippotask").exists(),
        "no store is created"
    );
}

/// The whole round trip, the way Claude Code runs it: the start hook writes
/// the env file, every Bash call sources it, the end hook gives work back.
#[cfg(unix)]
#[test]
fn a_session_claims_under_its_own_node_and_gives_it_back_at_the_end() {
    let dir = project("hook-round-trip");
    let env_file = dir.path().join("claude.env");
    let file = env_file.to_str().unwrap();
    let run = hook(
        dir.path(),
        "session-start",
        SESSION,
        &[("CLAUDE_ENV_FILE", file)],
    );
    assert_eq!(run.code, 0, "{run:#?}");
    let bash = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            ". '{file}' && '{}' start 1 --json",
            env!("CARGO_BIN_EXE_hippo-task")
        ))
        .current_dir(dir.path())
        .env_remove("HIPPO_ACTOR")
        .env_remove("HIPPO_NODE")
        .env_remove("HIPPO_DIR")
        .output()
        .unwrap();
    assert!(bash.status.success(), "{bash:#?}");
    assert_eq!(
        holder(dir.path(), "1").as_deref(),
        Some(format!("agent:claude@{NODE}").as_str())
    );
    let run = hook(dir.path(), "session-end", SESSION, &[]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(holder(dir.path(), "1"), None);
}

// ---- Codex (ADR-015) ----
//
// Codex has no env file: its SessionStart hook's stdout becomes the agent's
// context, so the start hook *tells* the session its identity. Codex session
// ids are UUIDv7 — time first — so the node is drawn from their random tail.

const CODEX_SESSION: &str = r#"{"session_id":"01a1363f-0a73-7180-ad3c-fbdddacabf3f","hook_event_name":"SessionStart","source":"startup","model":"gpt-5"}"#;
const CODEX_NODE: &str = "codex-dacabf3f";
/// Started in the same minute: the same first 8 characters.
const CODEX_NEIGHBOUR: &str = r#"{"session_id":"01a1363f-1b22-7d41-9e01-0c4f5a6b7c8d","hook_event_name":"SessionStart","source":"startup"}"#;

#[test]
fn codex_session_start_tells_the_agent_its_identity() {
    let dir = project("hook-codex-start");
    let run = codex_hook(dir.path(), "session-start", CODEX_SESSION, &[]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert!(
        run.stdout
            .contains(&format!("HIPPO_ACTOR=agent:codex HIPPO_NODE={CODEX_NODE}")),
        "{run:#?}"
    );
    assert_eq!(
        run.stdout.lines().count(),
        1,
        "one line of context: {run:#?}"
    );
    // --json changes nothing: the line is for Codex, which reads plain text.
    let json = hook_args(
        dir.path(),
        &["session-start", "--agent", "codex", "--json"],
        CODEX_SESSION,
        &[],
    );
    assert_eq!(json.stdout, run.stdout, "{json:#?}");
}

#[test]
fn codex_sessions_started_together_get_different_nodes() {
    let dir = project("hook-codex-neighbours");
    let first = codex_hook(dir.path(), "session-start", CODEX_SESSION, &[]);
    let second = codex_hook(dir.path(), "session-start", CODEX_NEIGHBOUR, &[]);
    assert!(
        second.stdout.contains("HIPPO_NODE=codex-5a6b7c8d"),
        "{second:#?}"
    );
    assert_ne!(first.stdout, second.stdout);
}

#[test]
fn codex_session_start_is_quiet_when_there_is_nothing_to_say() {
    // A project without tasks: no context spent on hippo-task.
    let plain = TempDir::new("hook-codex-plain");
    fs::create_dir(plain.path().join(".git")).unwrap();
    let run = codex_hook(plain.path(), "session-start", CODEX_SESSION, &[]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(run.stdout, "", "{run:#?}");
    assert!(!plain.path().join(".hippotask").exists());

    // Launched with an identity: every shell already has it.
    let dir = project("hook-codex-launched");
    let run = codex_hook(
        dir.path(),
        "session-start",
        CODEX_SESSION,
        &[("HIPPO_ACTOR", "agent:codex"), ("HIPPO_NODE", "qa-7")],
    );
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(run.stdout, "", "{run:#?}");

    // Launched with only an actor: the node is filled in, the actor kept.
    let run = codex_hook(
        dir.path(),
        "session-start",
        CODEX_SESSION,
        &[("HIPPO_ACTOR", "agent:reviewer")],
    );
    assert!(
        run.stdout.contains("agent:reviewer")
            && run.stdout.contains(&format!("HIPPO_NODE={CODEX_NODE}")),
        "{run:#?}"
    );
}

#[test]
fn codex_session_start_finds_the_store_from_the_sessions_folder() {
    let dir = project("hook-codex-cwd");
    let elsewhere = TempDir::new("hook-codex-elsewhere");
    let input = format!(
        r#"{{"session_id":"01a1363f-0a73-7180-ad3c-fbdddacabf3f","cwd":{}}}"#,
        serde_json::to_string(dir.path().to_str().unwrap()).unwrap()
    );
    let run = codex_hook(elsewhere.path(), "session-start", &input, &[]);
    assert!(run.stdout.contains(CODEX_NODE), "{run:#?}");
}

#[test]
fn codex_session_end_gives_back_what_this_session_holds_and_nothing_else() {
    let dir = project("hook-codex-end");
    ok(dir.path(), ("agent:codex", CODEX_NODE), &["start", "1"]);
    ok(
        dir.path(),
        ("agent:codex", "codex-5a6b7c8d"),
        &["start", "2"],
    );
    let run = codex_hook(dir.path(), "session-end", CODEX_SESSION, &[]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(run.stdout, "", "{run:#?}");
    assert_eq!(holder(dir.path(), "1"), None);
    assert_eq!(
        holder(dir.path(), "2").as_deref(),
        Some("agent:codex@codex-5a6b7c8d"),
        "the session started in the same minute keeps its claim"
    );
}

/// The whole round trip, the way a Codex agent does it: read the start
/// line, put the identity in front of a command, and the end hook gives the
/// work back.
#[cfg(unix)]
#[test]
fn a_codex_session_claims_under_the_identity_it_was_told() {
    let dir = project("hook-codex-round-trip");
    let told = codex_hook(dir.path(), "session-start", CODEX_SESSION, &[]);
    let prefix = told
        .stdout
        .split_whitespace()
        .filter(|word| word.starts_with("HIPPO_ACTOR=") || word.starts_with("HIPPO_NODE="))
        .map(|word| word.trim_end_matches(['.', ',', ';']))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        prefix,
        format!("HIPPO_ACTOR=agent:codex HIPPO_NODE={CODEX_NODE}"),
        "{told:#?}"
    );
    let shell = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "{prefix} '{}' start 1 --json",
            env!("CARGO_BIN_EXE_hippo-task")
        ))
        .current_dir(dir.path())
        .env_remove("HIPPO_ACTOR")
        .env_remove("HIPPO_NODE")
        .env_remove("HIPPO_DIR")
        .output()
        .unwrap();
    assert!(shell.status.success(), "{shell:#?}");
    assert_eq!(
        holder(dir.path(), "1").as_deref(),
        Some(format!("agent:codex@{CODEX_NODE}").as_str())
    );
    let run = codex_hook(dir.path(), "session-end", CODEX_SESSION, &[]);
    assert_eq!(run.code, 0, "{run:#?}");
    assert_eq!(holder(dir.path(), "1"), None);
}
