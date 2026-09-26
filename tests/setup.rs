//! Where tasks live (ADR-005): `hippo-task init` chooses a store once, and every
//! command finds it from anywhere in the project — never creating one by
//! accident. Plus `hippo-task guide`, the agent protocol from the binary.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{Run, TempDir};
use hippo_task::setup::{self, Place};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

const HUMAN: (&str, &str) = ("human:tester", "n0");
const CLAUDE: (&str, &str) = ("agent:claude", "cc");

/// Run the real binary *from* `cwd`, with no `HIPPO_DIR` — so it has to find
/// (or refuse to guess) the store itself.
fn at(cwd: &Path, who: (&str, &str), args: &[&str]) -> Run {
    let out = common::bare()
        .args(args)
        .current_dir(cwd)
        .env("HIPPO_ACTOR", who.0)
        .env("HIPPO_NODE", who.1)
        .output()
        .expect("run the hippo-task binary");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn ok_at(cwd: &Path, who: (&str, &str), args: &[&str]) -> Run {
    let run = at(cwd, who, args);
    assert_eq!(run.code, 0, "expected success for {args:?}:\n{run:#?}");
    run
}

/// A temp folder that is the root of a git repository, so the store search
/// stops here instead of wandering up the machine.
fn repo(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    fs::create_dir(dir.path().join(".git")).unwrap();
    dir
}

fn real(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap()
}

fn ledgers_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n == "ledger.jsonl") {
                found.push(path);
            }
        }
    }
    found
}

// ---- no store is ever created by accident ----

#[test]
fn outside_a_set_up_project_commands_say_to_run_init() {
    let dir = repo("setup-none");
    for args in [&["list"][..], &["add", "a task"][..]] {
        let run = at(dir.path(), HUMAN, args);
        assert_eq!(run.code, 2, "{args:?}: {run:#?}");
        assert!(run.stderr.contains("hippo-task init"), "{run:#?}");
    }
    assert!(
        !dir.path().join(".hippotask").exists(),
        "nothing was created"
    );
}

#[test]
fn guide_prints_the_agent_protocol_without_a_store_or_an_identity() {
    let dir = repo("setup-guide");
    // An agent with no node can still read the guide — it tells them to set one.
    let run = common::bare()
        .arg("guide")
        .current_dir(dir.path())
        .env("HIPPO_ACTOR", "agent:claude")
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0));
    let text = String::from_utf8_lossy(&run.stdout);
    for needle in [
        "The loop:",
        "list --json --ready",
        "release --all",
        "--search",
    ] {
        assert!(text.contains(needle), "guide lacks {needle:?}:\n{text}");
    }
    let json = ok_at(dir.path(), HUMAN, &["guide", "--json"]).json();
    assert!(json["guide"].as_str().unwrap().contains("The loop:"));
}

// ---- init: choose once ----

#[test]
fn init_here_in_a_git_repository_keeps_tasks_out_of_git_by_default() {
    let dir = repo("setup-here");
    let report = ok_at(dir.path(), HUMAN, &["init", "--here", "--json"]).json();
    let store = dir.path().join(".hippotask");
    assert_eq!(report["store"]["kind"], "local");
    assert_eq!(
        PathBuf::from(report["store"]["path"].as_str().unwrap()),
        real(&store)
    );
    assert_eq!(report["created"], true);
    assert_eq!(report["git"]["kept_out"], true);
    assert_eq!(
        fs::read_to_string(store.join(".gitignore"))
            .unwrap()
            .trim_end()
            .lines()
            .last(),
        Some("*"),
        "a .gitignore inside the store keeps the whole folder out of git"
    );

    ok_at(dir.path(), CLAUDE, &["add", "first task"]);
    assert_eq!(ledgers_under(dir.path()), [store.join("ledger.jsonl")]);
}

#[test]
fn init_can_keep_tasks_in_git_and_says_what_that_means() {
    let dir = repo("setup-keep");
    let run = ok_at(dir.path(), HUMAN, &["init", "--here", "--keep-in-git"]);
    assert!(
        run.stdout.contains("anyone who can read"),
        "the risk is spelled out: {run:#?}"
    );
    assert!(!dir.path().join(".hippotask/.gitignore").exists());
}

#[test]
fn init_tells_the_person_how_to_set_up_their_agents() {
    let dir = repo("setup-next");
    let run = ok_at(dir.path(), HUMAN, &["init", "--here"]);
    for needle in [
        "hippo-task guide",
        "HIPPO_NODE=",
        "SessionEnd",
        "hippo-task release --all",
    ] {
        assert!(
            run.stdout.contains(needle),
            "init lacks {needle:?}:\n{}",
            run.stdout
        );
    }
}

#[test]
fn init_needs_a_choice_when_it_cannot_ask() {
    let dir = repo("setup-no-tty");
    let run = at(dir.path(), HUMAN, &["init"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(
        run.stderr.contains("--here") && run.stderr.contains("--folder"),
        "{run:#?}"
    );
    assert!(!dir.path().join(".hippotask").exists());
}

#[test]
fn init_again_reports_the_store_and_never_moves_it() {
    let dir = repo("setup-again");
    ok_at(dir.path(), HUMAN, &["init", "--here"]);
    ok_at(dir.path(), CLAUDE, &["add", "keep me"]);
    let again = ok_at(dir.path(), HUMAN, &["init", "--here", "--json"]).json();
    assert_eq!(again["created"], false);

    let elsewhere = TempDir::new("setup-again-elsewhere");
    let folder = elsewhere.path().to_str().unwrap();
    let moved = at(dir.path(), HUMAN, &["init", "--folder", folder]);
    assert_eq!(moved.code, 2, "moving a ledger isn't supported: {moved:#?}");
    assert_eq!(
        ok_at(dir.path(), HUMAN, &["list", "--json"]).json()[0]["title"],
        "keep me"
    );
}

#[test]
fn init_from_a_subfolder_sets_up_the_repository_root() {
    let dir = repo("setup-subfolder-init");
    let deep = dir.path().join("src/deep");
    fs::create_dir_all(&deep).unwrap();
    ok_at(&deep, HUMAN, &["init", "--here"]);
    assert!(dir.path().join(".hippotask").is_dir());
    assert!(!deep.join(".hippotask").exists());
}

// ---- found from anywhere in the project ----

#[test]
fn commands_find_the_store_from_any_subfolder() {
    let dir = repo("setup-walk");
    ok_at(dir.path(), HUMAN, &["init", "--here"]);
    ok_at(dir.path(), HUMAN, &["add", "filed at the root"]);
    let deep = dir.path().join("src/deep");
    fs::create_dir_all(&deep).unwrap();

    let listed = ok_at(&deep, CLAUDE, &["list", "--json"]).json();
    assert_eq!(listed[0]["title"], "filed at the root");
    ok_at(&deep, CLAUDE, &["add", "filed from src/deep"]);
    assert_eq!(
        ledgers_under(dir.path()),
        [dir.path().join(".hippotask/ledger.jsonl")],
        "one ledger, however deep the agent works"
    );
}

#[test]
fn the_search_never_climbs_out_of_a_repository() {
    // A store *above* a repository belongs to something else.
    let outer = TempDir::new("setup-outer");
    ok_at(outer.path(), HUMAN, &["init", "--here"]);
    let inner = outer.path().join("repo");
    fs::create_dir_all(inner.join(".git")).unwrap();
    assert_eq!(at(&inner, HUMAN, &["list"]).code, 2);

    // …but a plain subfolder of the outer project still finds it.
    let plain = outer.path().join("notes");
    fs::create_dir_all(&plain).unwrap();
    ok_at(&plain, HUMAN, &["list"]);
}

// ---- another folder, via a pointer ----

#[test]
fn init_folder_points_the_project_at_another_folder() {
    let project = repo("setup-pointer");
    let elsewhere = TempDir::new("setup-pointer-store");
    let folder = elsewhere.path().join("tasks");
    let report = ok_at(
        project.path(),
        HUMAN,
        &["init", "--folder", folder.to_str().unwrap(), "--json"],
    )
    .json();
    assert_eq!(
        PathBuf::from(report["store"]["path"].as_str().unwrap()),
        real(&folder)
    );

    let pointer: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(project.path().join(".hippotask/store.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(pointer["version"], 1);
    assert_eq!(pointer["kind"], "local");
    assert_eq!(
        PathBuf::from(pointer["path"].as_str().unwrap()),
        real(&folder)
    );
    assert!(
        project.path().join(".hippotask/.gitignore").exists(),
        "the pointer names a path on this machine, so it's always kept out of git"
    );
    assert!(
        !folder.join(".gitignore").exists(),
        "that folder isn't in a repository"
    );

    ok_at(project.path(), CLAUDE, &["add", "lives elsewhere"]);
    assert_eq!(ledgers_under(project.path()), Vec::<PathBuf>::new());
    assert_eq!(ledgers_under(&folder), [folder.join("ledger.jsonl")]);
    // HIPPO_DIR, naming the project explicitly, follows the pointer too.
    let listed = common::ok(project.path(), HUMAN, &["list", "--json"]).json();
    assert_eq!(listed[0]["title"], "lives elsewhere");
}

#[test]
fn a_store_folder_inside_another_repository_is_kept_out_of_it() {
    let project = repo("setup-pointer-git");
    let notes = repo("setup-notes-repo");
    let folder = notes.path().join("hippo");
    ok_at(
        project.path(),
        HUMAN,
        &["init", "--folder", folder.to_str().unwrap()],
    );
    assert!(folder.join(".gitignore").exists());
}

#[test]
fn a_pointer_this_version_cannot_open_is_a_clear_error() {
    let project = repo("setup-hosted");
    fs::create_dir(project.path().join(".hippotask")).unwrap();
    fs::write(
        project.path().join(".hippotask/store.json"),
        r#"{"version": 1, "kind": "hosted", "url": "https://tasks.example.invalid/w/1"}"#,
    )
    .unwrap();
    let run = at(project.path(), HUMAN, &["list"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(
        run.stderr.contains("hosted") && run.stderr.contains("upgrade"),
        "{run:#?}"
    );
}

#[test]
fn a_pointer_to_a_missing_folder_says_so() {
    let project = repo("setup-gone");
    fs::create_dir(project.path().join(".hippotask")).unwrap();
    let gone = project.path().join("no-such-folder");
    let pointer = serde_json::json!({"version": 1, "kind": "local", "path": gone});
    fs::write(
        project.path().join(".hippotask/store.json"),
        pointer.to_string(),
    )
    .unwrap();
    let run = at(project.path(), HUMAN, &["list"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.stderr.contains("doesn't exist"), "{run:#?}");
}

// ---- the interactive prompt (library level, so no terminal is needed) ----

fn ask(project: &Path, answers: &str) -> setup::Choice {
    let mut out = Vec::new();
    setup::ask(project, &mut Cursor::new(answers), &mut out).unwrap()
}

#[test]
fn the_prompt_defaults_to_this_project_and_out_of_git() {
    let dir = repo("setup-prompt-default");
    let choice = ask(dir.path(), "\n\n");
    assert!(matches!(choice.place, Place::Here));
    assert!(choice.keep_out_of_git);
}

#[test]
fn the_prompt_asks_about_git_only_inside_a_repository() {
    let plain = TempDir::new("setup-prompt-plain");
    let mut out = Vec::new();
    let choice = setup::ask(plain.path(), &mut Cursor::new("1\n"), &mut out).unwrap();
    assert!(matches!(choice.place, Place::Here));
    assert!(!String::from_utf8(out).unwrap().contains("git"));

    let dir = repo("setup-prompt-git");
    let choice = ask(dir.path(), "1\nn\n");
    assert!(
        !choice.keep_out_of_git,
        "the person chose to keep tasks in git"
    );
}

#[test]
fn the_prompt_takes_another_folder() {
    let dir = repo("setup-prompt-folder");
    let elsewhere = TempDir::new("setup-prompt-elsewhere");
    let choice = ask(dir.path(), &format!("2\n{}\n", elsewhere.path().display()));
    match choice.place {
        Place::Folder(path) => assert_eq!(path, elsewhere.path()),
        Place::Here => panic!("expected another folder"),
    }
}

#[test]
fn the_prompt_refuses_an_unknown_choice() {
    let dir = repo("setup-prompt-bad");
    let mut out = Vec::new();
    assert!(setup::ask(dir.path(), &mut Cursor::new("3\n"), &mut out).is_err());
}
