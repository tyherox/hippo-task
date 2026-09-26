//! The CLI contract, tested by running the real binary: exit codes, JSON
//! shapes, text output, concurrency, crash tolerance, and privacy.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{bare, cmd, ok, tasks, TempDir, CLAUDE, CODEX, HUMAN};
use serde_json::Value;
use std::io::Read;
use std::process::{Child, Stdio};

// ---------------------------------------------------------------- golden path

#[test]
fn golden_path_two_agents_one_task() {
    let dir = TempDir::new("cli-golden");
    let d = dir.path();

    let added = ok(
        d,
        HUMAN,
        &[
            "add",
            "Ship auth",
            "--priority",
            "high",
            "--label",
            "backend",
            "--json",
        ],
    )
    .json();
    assert_eq!(added["num"], 1);
    assert_eq!(added["title"], "Ship auth");
    assert_eq!(added["state"], "todo");
    assert_eq!(added["labels"], serde_json::json!(["backend"]));
    ok(d, HUMAN, &["add", "Write docs", "--json"]);
    ok(d, HUMAN, &["update", "2", "--block", "1"]);

    // claude claims #1; codex is told to back off (exit 4), then takes nothing.
    let started = ok(d, CLAUDE, &["start", "1", "--json"]).json();
    assert_eq!(started["state"], "doing");
    assert_eq!(started["lease"]["holder"], "agent:claude");
    assert_eq!(started["lease"]["node"], "cc");
    assert_eq!(started["lease"]["active"], true);
    let refused = tasks(d, CODEX, &["start", "1"]);
    assert_eq!(refused.code, 4, "{refused:#?}");
    assert!(refused.stderr.contains("back off"), "{refused:#?}");
    assert!(refused.stdout.is_empty());

    // codex can't close claude's task either.
    assert_eq!(tasks(d, CODEX, &["done", "1"]).code, 4);

    ok(d, CLAUDE, &["note", "1", "token refresh fixed"]);
    let list = ok(d, HUMAN, &["list", "--json"]).json();
    assert_eq!(list.as_array().map(Vec::len), Some(2));
    assert_eq!(list[1]["blocked"], true, "#2 is blocked by the open #1");

    let done = ok(d, CLAUDE, &["done", "1", "--json"]).json();
    assert_eq!(done["state"], "done");
    assert_eq!(done["lease"], Value::Null, "done releases the lease");
    let two = ok(d, HUMAN, &["show", "2", "--json"]).json();
    assert_eq!(two["blocked"], false, "unblocked the moment #1 closed");

    // The audit trail keeps the rejected attempt, marked as such.
    let one = ok(d, HUMAN, &["show", "1", "--json"]).json();
    let events = one["events"].as_array().expect("events array");
    let rejected: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "lease" && e["applied"] == false)
        .collect();
    assert_eq!(rejected.len(), 1, "{events:#?}");
    assert_eq!(rejected[0]["actor"], "agent:codex");
    let text = ok(d, HUMAN, &["show", "1"]).stdout;
    assert!(text.contains("(rejected)"), "{text}");
    assert!(text.contains("\"token refresh fixed\""), "{text}");
}

// ---------------------------------------------------------------- regressions

#[test]
fn labels_given_to_add_are_never_lost() {
    // v0.0.1 lost labels ~70% of the time: the create and its labels shared a
    // millisecond, and a label could sort *before* the create.
    let dir = TempDir::new("cli-labels");
    for i in 0..30 {
        let title = format!("t{i}");
        ok(
            dir.path(),
            HUMAN,
            &[
                "add", &title, "--label", "a", "--label", "b", "--label", "c",
            ],
        );
    }
    let list = ok(dir.path(), HUMAN, &["list", "--json"]).json();
    for t in list.as_array().expect("array") {
        assert_eq!(t["labels"], serde_json::json!(["a", "b", "c"]), "{t}");
    }
}

#[test]
fn a_lease_taken_right_after_create_is_not_dropped() {
    // Same bug, across processes: a lease stamped in the create's millisecond
    // by a node that sorts first used to vanish.
    let dir = TempDir::new("cli-fast-lease");
    for i in 1..=20 {
        let n = i.to_string();
        ok(dir.path(), ("human:h", "zz"), &["add", "x"]);
        ok(dir.path(), ("agent:a", "aa"), &["lease", &n]);
        let t = ok(dir.path(), HUMAN, &["show", &n, "--json"]).json();
        assert_eq!(t["lease"]["holder"], "agent:a", "task {n}: {t}");
    }
}

#[test]
fn a_torn_last_line_does_not_swallow_the_next_task() {
    let dir = TempDir::new("cli-torn");
    ok(dir.path(), HUMAN, &["add", "before crash"]);
    append_bytes(&dir, br#"{"eid":"01TORN","ta"#);
    ok(dir.path(), HUMAN, &["add", "after crash"]);

    let run = ok(dir.path(), HUMAN, &["list"]);
    assert!(run.stdout.contains("after crash"), "{run:#?}");
    assert!(
        run.stderr.contains("skipped an unreadable line"),
        "reported, not hidden: {run:#?}"
    );
}

#[test]
fn a_garbage_line_is_a_warning_not_a_crash() {
    let dir = TempDir::new("cli-garbage");
    ok(dir.path(), HUMAN, &["add", "survivor"]);
    append_bytes(&dir, b"\xff\xfe not utf-8 \n");
    let run = ok(dir.path(), HUMAN, &["list"]);
    assert!(run.stdout.contains("survivor"));
    assert!(run.stderr.contains("warning:"), "{run:#?}");

    let json = ok(dir.path(), HUMAN, &["list", "--json"]);
    let warning: Value = serde_json::from_str(json.stderr.trim()).expect("JSON warning line");
    assert!(warning["warning"].is_string(), "{json:#?}");
}

#[test]
fn a_closed_stdout_is_a_clean_exit_not_a_panic() {
    // `hippo-task list | head -1` on a big list used to panic with "Broken pipe".
    let dir = TempDir::new("cli-pipe");
    let mut text = String::new();
    for i in 0..20_000 {
        text.push_str(&format!(
            r#"{{"eid":"E{i:06}","task":"T{i:06}","ts":{i},"actor":"a","node":"n","type":"create","data":{{"title":"task {i}","priority":"none","body":null,"assignee":null}}}}"#
        ));
        text.push('\n');
    }
    std::fs::create_dir_all(dir.path().join(".hippotask")).unwrap();
    std::fs::write(dir.ledger(), text).unwrap();

    let mut child = cmd(dir.path(), HUMAN, &["list"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut first = [0u8; 64];
    child.stdout.take().unwrap().read_exact(&mut first).unwrap(); // read a little, then hang up
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "exit {:?}: {stderr}",
        out.status.code()
    );
    assert!(!stderr.contains("panicked"), "{stderr}");
}

// ---------------------------------------------------------------- exit codes

#[test]
fn exit_codes_are_the_contract() {
    let dir = TempDir::new("cli-exit");
    let d = dir.path();
    ok(d, HUMAN, &["add", "x"]);

    let code = |args: &[&str]| tasks(d, HUMAN, args).code;
    assert_eq!(code(&["show", "1"]), 0);
    assert_eq!(code(&["list"]), 0);
    // 2 — usage
    assert_eq!(code(&["add", "  "]), 2, "empty title");
    assert_eq!(code(&["update", "1"]), 2, "nothing to update");
    assert_eq!(code(&["update", "1", "--block", "1"]), 2, "self-block");
    assert_eq!(code(&["lease", "1", "--minutes", "0"]), 2);
    assert_eq!(
        code(&["lease", "1", "--minutes", "9223372036854775807"]),
        2,
        "used to overflow and panic"
    );
    assert_eq!(code(&["show", "ab"]), 2, "too short to be an id");
    assert_eq!(code(&["list", "--sort", "sideways"]), 2);
    assert_eq!(code(&["frobnicate"]), 2);
    // 3 — not found
    assert_eq!(code(&["show", "9"]), 3);
    assert_eq!(
        code(&["update", "1", "--block", "99"]),
        3,
        "used to create a dangling relation"
    );
    // 4 — conflict
    ok(d, CLAUDE, &["lease", "1"]);
    assert_eq!(tasks(d, CODEX, &["lease", "1"]).code, 4);
    // 2 — usage, too: --dir must name an existing directory (1 = io is tested below)
    let a_file = d.join("a-file");
    std::fs::write(&a_file, "not a directory").unwrap();
    assert_eq!(
        tasks(&a_file, HUMAN, &["list"]).code,
        2,
        "a file is not a directory"
    );
    assert_eq!(
        tasks(&d.join("missing"), HUMAN, &["add", "x"]).code,
        2,
        "--dir must exist"
    );
}

#[test]
fn an_unwritable_ledger_is_an_io_error() {
    let dir = TempDir::new("cli-io");
    ok(dir.path(), HUMAN, &["add", "x"]);
    // Replace the ledger with a directory: opening it for append fails.
    std::fs::remove_file(dir.ledger()).unwrap();
    std::fs::create_dir(dir.ledger()).unwrap();
    let run = tasks(dir.path(), HUMAN, &["add", "y"]);
    assert_eq!(run.code, 1, "{run:#?}");
    assert!(run.stderr.starts_with("error: couldn't"), "{run:#?}");
}

#[test]
fn json_errors_are_one_json_line_on_stderr() {
    let dir = TempDir::new("cli-json-err");
    let run = tasks(dir.path(), HUMAN, &["show", "7", "--json"]);
    assert_eq!(run.code, 3);
    assert!(run.stdout.is_empty());
    let err = run.json_error();
    assert_eq!(err["error"], "not_found");
    assert_eq!(err["exit_code"], 3);
    assert!(err["message"].as_str().is_some_and(|m| m.contains('7')));

    // Even argument-parsing errors are JSON in --json mode.
    let run = tasks(dir.path(), HUMAN, &["list", "--json", "--bogus"]);
    assert_eq!(run.code, 2);
    assert_eq!(run.json_error()["error"], "usage");
}

#[test]
fn help_and_version_exit_zero_and_document_the_contract() {
    let help = bare().arg("--help").output().unwrap();
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    for needle in ["Exit codes", "4 conflict", "--json", "AGENTS.md"] {
        assert!(text.contains(needle), "--help lacks {needle:?}:\n{text}");
    }
    let version = bare().arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));
}

// ---------------------------------------------------------------- coordination

#[test]
fn two_windows_of_the_same_agent_are_excluded_by_the_lease() {
    // The first real use: several chat windows of the same provider.
    let dir = TempDir::new("cli-windows");
    ok(dir.path(), HUMAN, &["add", "x"]);
    ok(dir.path(), ("agent:claude", "window-1"), &["start", "1"]);
    let second = tasks(dir.path(), ("agent:claude", "window-2"), &["start", "1"]);
    assert_eq!(second.code, 4, "{second:#?}");
    assert!(
        second.stderr.contains("agent:claude@window-1"),
        "names the holder: {second:#?}"
    );
}

#[test]
fn closed_tasks_cannot_be_leased_and_never_show_a_lease() {
    let dir = TempDir::new("cli-closed");
    ok(dir.path(), HUMAN, &["add", "x"]);
    ok(dir.path(), CLAUDE, &["start", "1"]);
    ok(dir.path(), CLAUDE, &["update", "1", "--state", "done"]);
    let t = ok(dir.path(), HUMAN, &["show", "1", "--json"]).json();
    assert_eq!(t["lease"], Value::Null);
    let again = tasks(dir.path(), CODEX, &["start", "1"]);
    assert_eq!(again.code, 4);
    assert!(
        again.stderr.contains("--state todo"),
        "tells you how to reopen: {again:#?}"
    );
}

#[test]
fn force_lets_a_human_close_a_task_a_crashed_agent_holds() {
    let dir = TempDir::new("cli-force");
    ok(dir.path(), HUMAN, &["add", "x"]);
    ok(dir.path(), CLAUDE, &["start", "1"]);
    assert_eq!(tasks(dir.path(), HUMAN, &["done", "1"]).code, 4);
    let t = ok(dir.path(), HUMAN, &["done", "1", "--force", "--json"]).json();
    assert_eq!(t["state"], "done");
}

#[test]
fn concurrent_writers_lose_nothing_and_stay_in_time_order() {
    let dir = TempDir::new("cli-concurrent");
    ok(dir.path(), HUMAN, &["list"]);
    let writers: Vec<Child> = (0..8)
        .map(|w| {
            let script = format!(
                "for i in $(seq 1 25); do \"$0\" add \"w{w}-$i\" --label w{w} >/dev/null || exit 1; done"
            );
            std::process::Command::new("sh")
                .arg("-c")
                .arg(script)
                .arg(env!("CARGO_BIN_EXE_hippo-task"))
                .env("HIPPO_DIR", dir.path())
                .env("HIPPO_ACTOR", format!("agent:w{w}"))
                .env("HIPPO_NODE", format!("node{w}"))
                .spawn()
                .unwrap()
        })
        .collect();
    for mut w in writers {
        assert!(w.wait().unwrap().success());
    }

    let list = ok(dir.path(), HUMAN, &["list", "--json"]).json();
    let list = list.as_array().expect("array");
    assert_eq!(list.len(), 200);
    let nums: Vec<u64> = list.iter().filter_map(|t| t["num"].as_u64()).collect();
    assert_eq!(nums, (1..=200).collect::<Vec<u64>>());
    assert!(list
        .iter()
        .all(|t| t["labels"].as_array().map(Vec::len) == Some(1)));

    // Ledger order == time order: every timestamp strictly greater than the last.
    let ts: Vec<i64> = dir
        .ledger_text()
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).expect("valid line")["ts"]
                .as_i64()
                .expect("ts")
        })
        .collect();
    assert_eq!(ts.len(), 400, "200 creates + 200 labels, no torn lines");
    assert!(
        ts.windows(2).all(|w| w[0] < w[1]),
        "timestamps must strictly increase in file order"
    );
}

#[test]
fn a_lease_race_has_exactly_one_winner_and_it_is_the_real_holder() {
    let dir = TempDir::new("cli-race");
    for round in 1..=6 {
        let id = round.to_string();
        ok(dir.path(), HUMAN, &["add", "contested"]);
        let racers: Vec<(String, Child)> = (0..10)
            .map(|r| {
                let actor = format!("agent:r{r}");
                let child = cmd(dir.path(), (&actor, &format!("node{r}")), &["lease", &id])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap();
                (actor, child)
            })
            .collect();
        let mut winners = Vec::new();
        for (actor, mut child) in racers {
            match child.wait().unwrap().code() {
                Some(0) => winners.push(actor),
                Some(4) => {}
                other => panic!("round {round}: unexpected exit {other:?}"),
            }
        }
        assert_eq!(winners.len(), 1, "round {round}: winners {winners:?}");
        let t = ok(dir.path(), HUMAN, &["show", &id, "--json"]).json();
        assert_eq!(t["lease"]["holder"], winners[0].as_str(), "round {round}");
    }
}

// ---------------------------------------------------------------- privacy

#[test]
fn no_personal_data_is_recorded_by_default() {
    // PII rule: nothing about the person or machine is collected unless they opt in.
    let dir = TempDir::new("cli-pii");
    let canary = "zq-canary-7731";
    let run = |args: &[&str]| {
        let out = bare()
            .args(args)
            .env("HIPPO_DIR", dir.path())
            .env("USER", canary)
            .env("LOGNAME", canary)
            .env("USERNAME", canary)
            .env("HOSTNAME", canary)
            .env("NAME", canary)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let all = [out.stdout, out.stderr].concat();
        assert!(
            !String::from_utf8_lossy(&all).contains(canary),
            "{args:?} printed personal data"
        );
    };
    run(&["add", "a task", "--label", "x", "--body", "desc"]);
    run(&["add", "another"]);
    run(&["update", "1", "--priority", "high", "--block", "2"]);
    run(&["start", "1"]);
    run(&["note", "1", "progress"]);
    run(&["desc", "1", "new description"]);
    run(&["release", "1"]);
    run(&["lease", "2"]);
    run(&["done", "2"]);
    run(&["list"]);
    run(&["list", "--json"]);
    run(&["show", "1"]);
    run(&["show", "1", "--json"]);

    let ledger = dir.ledger_text();
    assert!(
        !ledger.contains(canary),
        "the ledger recorded personal data"
    );
    for line in ledger.lines() {
        let ev: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            ev["actor"], "human:local",
            "default actor carries no identity: {line}"
        );
        assert_eq!(
            ev["node"], "local",
            "default node carries no identity: {line}"
        );
        let keys: Vec<&str> = ev.as_object().unwrap().keys().map(String::as_str).collect();
        let allowed = ["eid", "task", "ts", "actor", "node", "type", "data"];
        assert!(
            keys.iter().all(|k| allowed.contains(k)),
            "unexpected field in {line}"
        );
    }
}

#[test]
fn only_what_you_opt_into_is_recorded() {
    let dir = TempDir::new("cli-optin");
    ok(dir.path(), ("human:ana", "laptop"), &["add", "x"]);
    let ledger = dir.ledger_text();
    assert!(ledger.contains(r#""actor":"human:ana""#));
    assert!(ledger.contains(r#""node":"laptop""#));
}

// ---------------------------------------------------------------- text output

#[test]
fn text_output_is_for_humans_and_says_what_happened() {
    let dir = TempDir::new("cli-text");
    let d = dir.path();
    let added = ok(
        d,
        HUMAN,
        &[
            "add",
            "Ship auth",
            "--priority",
            "urgent",
            "--label",
            "backend",
        ],
    );
    assert!(added.stdout.starts_with("added #1 "), "{added:#?}");
    ok(d, CLAUDE, &["start", "1"]);
    let line = ok(d, HUMAN, &["list"]).stdout;
    assert!(line.starts_with("#1"), "{line}");
    for needle in [
        "doing",
        "urgent",
        "Ship auth",
        "lease:agent:claude@cc(10m)",
        "backend",
    ] {
        assert!(line.contains(needle), "list lacks {needle:?}: {line}");
    }
    let released = ok(d, CODEX, &["release", "1"]).stdout;
    assert!(released.contains("nothing to release"), "{released}");
    let show = ok(d, HUMAN, &["show", "1"]).stdout;
    assert!(show.contains(" UTC"), "times are labelled: {show}");
}

// ---------------------------------------------------------------- JSON contract

#[test]
fn json_shapes_are_frozen_within_0_1_x() {
    // AGENTS.md promises that within 0.1.x, --json fields are only ever added:
    // removing or renaming one must fail here; adding one is a deliberate edit of this list.
    const TASK: [&str; 14] = [
        "id",
        "num",
        "title",
        "body",
        "state",
        "priority",
        "assignee",
        "labels",
        "relations",
        "blocked",
        "lease",
        "created_ms",
        "updated_ms",
        "seq",
    ];
    const LEASE: [&str; 4] = ["holder", "node", "expires_ms", "active"];
    const EVENT: [&str; 8] = [
        "eid", "task", "ts", "actor", "node", "type", "data", "applied",
    ];
    let dir = TempDir::new("cli-json-shape");
    let d = dir.path();

    let added = ok(d, HUMAN, &["add", "x", "--json"]).stdout;
    assert_eq!(keys_in_order(&added, ""), TASK);
    let started = ok(d, CLAUDE, &["start", "1", "--json"]).stdout;
    assert_eq!(keys_in_order(&started, ""), TASK);
    assert_eq!(keys_in_order(&started, "\"lease\":"), LEASE);
    let leased = ok(d, CLAUDE, &["lease", "1", "--json"]).stdout;
    assert_eq!(keys_in_order(&leased, "\"lease\":"), LEASE);
    let released = ok(d, CLAUDE, &["release", "1", "--json"]).stdout;
    let mut release_keys = TASK.to_vec();
    release_keys.push("released");
    assert_eq!(keys_in_order(&released, ""), release_keys);
    let shown = ok(d, HUMAN, &["show", "1", "--json"]).stdout;
    let mut detail_keys = TASK.to_vec();
    detail_keys.push("events");
    assert_eq!(keys_in_order(&shown, ""), detail_keys);
    assert_eq!(keys_in_order(&shown, "\"events\":"), EVENT);
    let list = ok(d, HUMAN, &["list", "--json"]).stdout;
    assert_eq!(
        keys_in_order(&list, ""),
        TASK,
        "list is an array of task objects"
    );
}

/// The keys of the first JSON object after `marker`, in printed order.
/// (serde_json's `Value` sorts keys, so the raw text is scanned instead.)
fn keys_in_order(json: &str, marker: &str) -> Vec<String> {
    let at = json
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not in {json}"));
    let rest = &json[at + marker.len()..];
    let obj = &rest[rest.find('{').expect("an object")..];
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut expect_key = false;
    let mut chars = obj.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('\\') => s.extend(chars.next()),
                        Some('"') | None => break,
                        Some(c) => s.push(c),
                    }
                }
                if depth == 1 && expect_key {
                    keys.push(s);
                    expect_key = false;
                }
            }
            '{' | '[' => {
                depth += 1;
                expect_key = depth == 1;
            }
            '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            ',' if depth == 1 => expect_key = true,
            _ => {}
        }
    }
    keys
}

#[test]
fn release_json_says_whether_you_held_the_lease() {
    let dir = TempDir::new("cli-release-json");
    let d = dir.path();
    ok(d, HUMAN, &["add", "x"]);
    ok(d, CLAUDE, &["start", "1"]);

    let by_codex = ok(d, CODEX, &["release", "1", "--json"]).json();
    assert_eq!(by_codex["released"], false);
    assert_eq!(
        by_codex["lease"]["holder"], "agent:claude",
        "codex can't release claude's lease"
    );
    let by_claude = ok(d, CLAUDE, &["release", "1", "--json"]).json();
    assert_eq!(by_claude["released"], true);
    assert_eq!(by_claude["lease"], Value::Null);
    assert_eq!(
        by_claude["state"], "doing",
        "release doesn't touch the state"
    );
}

// ---------------------------------------------------------------- closed tasks

#[test]
fn closed_tasks_can_be_reopened_and_still_take_notes_and_edits() {
    let dir = TempDir::new("cli-reopen");
    let d = dir.path();
    ok(d, HUMAN, &["add", "x"]);
    let done = ok(d, HUMAN, &["done", "1", "--json"]).json();
    let seq = done["seq"].as_u64().expect("seq");

    // Closing again is not an error: it's recorded, but changes nothing.
    let again = ok(d, HUMAN, &["done", "1", "--json"]).json();
    assert_eq!(again["seq"], seq);
    let shown = ok(d, HUMAN, &["show", "1", "--json"]).json();
    let completes: Vec<bool> = shown["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["type"] == "complete")
        .map(|e| e["applied"] == true)
        .collect();
    assert_eq!(completes, [true, false], "{shown:#?}");

    // Notes and metadata stay collaborative after closing.
    ok(d, CLAUDE, &["note", "1", "closed, but worth remembering"]);
    ok(d, CLAUDE, &["desc", "1", "what shipped"]);
    let edited = ok(
        d,
        CLAUDE,
        &["update", "1", "--label-add", "reviewed", "--json"],
    )
    .json();
    assert_eq!(edited["state"], "done");
    assert_eq!(edited["body"], "what shipped");
    assert_eq!(edited["labels"], serde_json::json!(["reviewed"]));

    // `update --state todo` reopens it — the way the lease refusal says to.
    let reopened = ok(d, HUMAN, &["update", "1", "--state", "todo", "--json"]).json();
    assert_eq!(reopened["state"], "todo");
    ok(d, CLAUDE, &["start", "1"]);
}

fn append_bytes(dir: &TempDir, bytes: &[u8]) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.ledger())
        .unwrap();
    f.write_all(bytes).unwrap();
}

// ---------------------------------------------------------------- review fixes (0.1.0)

#[test]
fn agents_must_name_their_node() {
    // A lease belongs to actor + node; a shared default node would make two
    // windows of one agent the same worker, and the lease would stop protecting them.
    let dir = TempDir::new("cli-agent-node");
    ok(dir.path(), HUMAN, &["add", "x"]);
    let out = bare()
        .args(["start", "1"])
        .env("HIPPO_DIR", dir.path())
        .env("HIPPO_ACTOR", "agent:claude")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("HIPPO_NODE"),
        "{out:?}"
    );
    assert!(
        !dir.ledger_text().contains("agent:claude"),
        "refused before writing anything"
    );

    // Humans keep the privacy-preserving default (human:local@local).
    let human = bare()
        .args(["list"])
        .env("HIPPO_DIR", dir.path())
        .output()
        .unwrap();
    assert!(human.status.success());
    // And an agent that names its node is fine.
    ok(dir.path(), ("agent:claude", "win-1"), &["start", "1"]);
}
