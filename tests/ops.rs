//! The commands' rules (ops.rs), through the library API with a controlled
//! clock — so lease expiry can be tested without sleeping.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::TempDir;
use hippo_task::error::Error;
use hippo_task::model::{Priority, State, Task};
use hippo_task::ops::{self, Changes, Ctx, Filter, NewTask, Sort};
use hippo_task::store::Store;

const MIN: i64 = 60_000;

fn ctx(actor: &str, node: &str, now_ms: i64) -> Ctx {
    Ctx {
        actor: actor.into(),
        node: node.into(),
        now_ms,
    }
}

fn human(now_ms: i64) -> Ctx {
    ctx("human:tester", "n0", now_ms)
}

fn new_task(title: &str) -> NewTask {
    NewTask {
        title: title.into(),
        priority: Priority::None,
        body: None,
        assignee: None,
        labels: vec![],
    }
}

fn add(store: &Store, title: &str) -> Task {
    ops::add(store, &human(0), new_task(title)).unwrap()
}

fn is_conflict<T: std::fmt::Debug>(r: Result<T, Error>) -> bool {
    matches!(r, Err(Error::Conflict(_)))
}

// ---- leases over time ----

#[test]
fn a_lease_blocks_others_until_it_expires() {
    let dir = TempDir::new("ops-expiry");
    let store = Store::new(dir.path());
    let t = add(&store, "Ship auth");

    ops::lease(&store, &ctx("agent:a", "na", 1_000), "1", 1).unwrap();
    assert!(is_conflict(ops::lease(
        &store,
        &ctx("agent:b", "nb", 1_000 + 30_000),
        "1",
        5
    )));
    let taken = ops::lease(&store, &ctx("agent:b", "nb", 1_000 + MIN + 1), "1", 5).unwrap();
    let lease = taken.lease.expect("b holds it now");
    assert_eq!(
        (lease.holder.as_str(), lease.node.as_str()),
        ("agent:b", "nb")
    );
    assert_eq!(taken.id, t.id);
}

#[test]
fn renewing_extends_your_own_lease() {
    let dir = TempDir::new("ops-renew");
    let store = Store::new(dir.path());
    add(&store, "Ship auth");
    let a = |now| ctx("agent:a", "na", now);

    let first = ops::lease(&store, &a(1_000), "1", 10).unwrap();
    let renewed = ops::lease(&store, &a(1_000 + 5 * MIN), "1", 10).unwrap();
    let expiry = |t: &Task| t.lease.as_ref().map(|l| l.expires_ms);
    assert!(expiry(&renewed) > expiry(&first));
}

#[test]
fn mine_lists_only_active_leases_held_by_this_worker() {
    let dir = TempDir::new("ops-mine");
    let store = Store::new(dir.path());
    add(&store, "one");
    add(&store, "two");
    ops::lease(&store, &ctx("agent:a", "na", 0), "1", 1).unwrap();
    ops::lease(&store, &ctx("agent:a", "other-window", 0), "2", 1).unwrap();

    let mine = Filter {
        mine: true,
        ..Filter::default()
    };
    let now = ops::list(&store, &ctx("agent:a", "na", 30_000), &mine).unwrap();
    assert_eq!(now.iter().map(|t| t.num).collect::<Vec<_>>(), [1]);
    let later = ops::list(&store, &ctx("agent:a", "na", 2 * MIN), &mine).unwrap();
    assert!(later.is_empty(), "an expired lease isn't yours any more");
}

#[test]
fn release_is_a_no_op_for_non_holders() {
    let dir = TempDir::new("ops-release");
    let store = Store::new(dir.path());
    add(&store, "x");
    ops::lease(&store, &ctx("agent:a", "na", 0), "1", 10).unwrap();

    let by_b = ops::release(&store, &ctx("agent:b", "nb", 1), "1").unwrap();
    assert!(!by_b.released);
    assert!(by_b.task.lease.is_some(), "b can't release a's lease");

    let by_a = ops::release(&store, &ctx("agent:a", "na", 2), "1").unwrap();
    assert!(by_a.released);
    assert!(by_a.task.lease.is_none());
}

#[test]
fn a_refused_start_records_only_the_attempt() {
    let dir = TempDir::new("ops-start");
    let store = Store::new(dir.path());
    add(&store, "x");
    ops::start(&store, &ctx("agent:a", "na", 0), "1", 10).unwrap();
    let before = store.read().unwrap().events.len();

    assert!(is_conflict(ops::start(
        &store,
        &ctx("agent:b", "nb", 1),
        "1",
        10
    )));
    let after = store.read().unwrap().events;
    assert_eq!(
        after.len(),
        before + 1,
        "the rejected lease is audited, nothing else"
    );
    let detail = ops::show(&store, "1").unwrap();
    let last = detail.history.last().unwrap();
    assert_eq!(last.event.actor, "agent:b");
    assert!(!last.applied);
    assert_eq!(detail.task.state, State::Doing);
}

// ---- closing etiquette ----

#[test]
fn only_the_holder_closes_a_leased_task_unless_forced() {
    let dir = TempDir::new("ops-close");
    let store = Store::new(dir.path());
    add(&store, "x");
    ops::start(&store, &ctx("agent:a", "na", 0), "1", 10).unwrap();
    let b = ctx("agent:b", "nb", 1);

    assert!(is_conflict(ops::done(&store, &b, "1", false)));
    let cancel = Changes {
        state: Some(State::Cancelled),
        ..Changes::default()
    };
    assert!(is_conflict(ops::update(&store, &b, "1", cancel.clone())));

    let forced = ops::update(
        &store,
        &b,
        "1",
        Changes {
            force: true,
            ..cancel
        },
    )
    .unwrap();
    assert_eq!(forced.state, State::Cancelled);
    assert!(forced.lease.is_none(), "closing clears the lease");
}

#[test]
fn the_holder_can_close_and_that_clears_the_lease() {
    let dir = TempDir::new("ops-holder-close");
    let store = Store::new(dir.path());
    add(&store, "x");
    let a = ctx("agent:a", "na", 0);
    ops::start(&store, &a, "1", 10).unwrap();
    let done = ops::update(
        &store,
        &a,
        "1",
        Changes {
            state: Some(State::Done),
            ..Changes::default()
        },
    )
    .unwrap();
    assert!(done.lease.is_none());
    assert!(
        is_conflict(ops::lease(&store, &ctx("agent:b", "nb", 1), "1", 10)),
        "closed tasks can't be leased"
    );
}

#[test]
fn an_expired_lease_does_not_protect_the_task() {
    let dir = TempDir::new("ops-expired-close");
    let store = Store::new(dir.path());
    add(&store, "x");
    ops::lease(&store, &ctx("agent:a", "na", 0), "1", 1).unwrap();
    let t = ops::done(&store, &ctx("agent:b", "nb", 2 * MIN), "1", false).unwrap();
    assert_eq!(t.state, State::Done);
}

// ---- input rules ----

#[test]
fn empty_input_is_a_usage_error() {
    let dir = TempDir::new("ops-empty");
    let store = Store::new(dir.path());
    let h = human(0);
    assert!(matches!(
        ops::add(&store, &h, new_task("   ")),
        Err(Error::Usage(_))
    ));
    let mut blank_label = new_task("ok");
    blank_label.labels = vec![" ".into()];
    assert!(matches!(
        ops::add(&store, &h, blank_label),
        Err(Error::Usage(_))
    ));

    add(&store, "x");
    assert!(matches!(
        ops::update(&store, &h, "1", Changes::default()),
        Err(Error::Usage(_))
    ));
    assert!(matches!(
        ops::note(&store, &h, "1", ""),
        Err(Error::Usage(_))
    ));
    assert!(matches!(
        ops::describe(&store, &h, "1", "  "),
        Err(Error::Usage(_))
    ));
    assert!(matches!(
        ops::lease(&store, &h, "1", 0),
        Err(Error::Usage(_))
    ));
    assert!(matches!(
        ops::lease(&store, &h, "1", 1_441),
        Err(Error::Usage(_))
    ));
}

#[test]
fn titles_and_labels_are_trimmed() {
    let dir = TempDir::new("ops-trim");
    let store = Store::new(dir.path());
    let mut nt = new_task("  Ship auth  ");
    nt.labels = vec![" backend ".into()];
    let t = ops::add(&store, &human(0), nt).unwrap();
    assert_eq!(t.title, "Ship auth");
    assert!(t.labels.contains("backend"));
}

#[test]
fn blocking_rules() {
    let dir = TempDir::new("ops-block");
    let store = Store::new(dir.path());
    add(&store, "blocker");
    add(&store, "blocked");
    let h = human(0);
    let block = |on: &str| Changes {
        block: vec![on.into()],
        ..Changes::default()
    };

    assert!(matches!(
        ops::update(&store, &h, "2", block("99")),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        ops::update(&store, &h, "2", block("2")),
        Err(Error::Usage(_))
    ));
    assert!(ops::update(&store, &h, "2", block("1")).unwrap().blocked);

    let unblock = Changes {
        unblock: vec!["1".into()],
        ..Changes::default()
    };
    assert!(!ops::update(&store, &h, "2", unblock).unwrap().blocked);
}

#[test]
fn unassign_clears_the_assignee() {
    let dir = TempDir::new("ops-unassign");
    let store = Store::new(dir.path());
    add(&store, "x");
    let h = human(0);
    let set = Changes {
        assignee: Some("agent:a".into()),
        ..Changes::default()
    };
    assert_eq!(
        ops::update(&store, &h, "1", set)
            .unwrap()
            .assignee
            .as_deref(),
        Some("agent:a")
    );
    let clear = Changes {
        unassign: true,
        ..Changes::default()
    };
    assert_eq!(ops::update(&store, &h, "1", clear).unwrap().assignee, None);
}

// ---- id resolution ----

/// A ledger with one task whose id we choose (so suffix tests are deterministic).
fn ledger_with_id(dir: &TempDir, id: &str) -> Store {
    std::fs::create_dir_all(dir.path().join(".hippotask")).unwrap();
    let line = format!(
        r#"{{"eid":"E1","task":"{id}","ts":1,"actor":"a","node":"n","type":"create","data":{{"title":"x","priority":"none","body":null,"assignee":null}}}}"#
    );
    std::fs::write(dir.ledger(), format!("{line}\n")).unwrap();
    Store::new(dir.path())
}

#[test]
fn ids_resolve_by_number_full_id_or_long_enough_suffix() {
    let dir = TempDir::new("ops-resolve");
    let id = "01K0000000000000000000XYZW";
    let store = ledger_with_id(&dir, id);
    let title = |input: &str| ops::show(&store, input).map(|d| d.task.title);

    assert_eq!(title("1").unwrap(), "x");
    assert_eq!(title("#1").unwrap(), "x");
    assert_eq!(title(id).unwrap(), "x");
    assert_eq!(
        title(&id.to_lowercase()).unwrap(),
        "x",
        "ULIDs are case-insensitive"
    );
    assert_eq!(
        title("XYZW").unwrap(),
        "x",
        "a 4-character suffix is enough"
    );
    assert_eq!(title("xyzw").unwrap(), "x");
    assert!(matches!(title("2"), Err(Error::NotFound(_))));
    assert!(matches!(title("ZZZZ"), Err(Error::NotFound(_))));
    assert!(
        matches!(title("YZW"), Err(Error::Usage(_))),
        "3 characters is too short"
    );
    assert!(matches!(title(""), Err(Error::Usage(_))));
}

#[test]
fn digits_are_always_a_task_number_never_a_suffix() {
    // A task whose id ends in digits: `12` must mean task #12, not "the id ending in 12".
    let dir = TempDir::new("ops-digits");
    let store = ledger_with_id(&dir, "01K00000000000000000000012");
    assert!(matches!(ops::show(&store, "12"), Err(Error::NotFound(_))));
    assert!(
        matches!(ops::show(&store, "0012"), Err(Error::NotFound(_))),
        "all digits → the number 12"
    );
    assert_eq!(ops::show(&store, "1").unwrap().task.num, 1);
}

#[test]
fn ambiguous_suffixes_are_refused() {
    // Two ids sharing a 4-char suffix.
    let dir = TempDir::new("ops-ambiguous");
    std::fs::create_dir_all(dir.path().join(".hippotask")).unwrap();
    let mk = |eid: &str, id: &str, ts: i64| {
        format!(
            r#"{{"eid":"{eid}","task":"{id}","ts":{ts},"actor":"a","node":"n","type":"create","data":{{"title":"t","priority":"none","body":null,"assignee":null}}}}"#
        )
    };
    let text = format!(
        "{}\n{}\n",
        mk("E1", "01K0000000000000000000ABCD", 1),
        mk("E2", "01K0000000000000000001ABCD", 2)
    );
    std::fs::write(dir.ledger(), text).unwrap();
    let store = Store::new(dir.path());
    let err = ops::show(&store, "ABCD").unwrap_err();
    assert!(matches!(err, Error::Usage(_)), "{err:?}");
    assert!(err.to_string().contains("#1, #2"), "{err}");
    assert_eq!(ops::show(&store, "1ABCD").unwrap().task.num, 2);
}

// ---- listing ----

#[test]
fn list_filters_and_sorts() {
    let dir = TempDir::new("ops-list");
    let store = Store::new(dir.path());
    let h = human(0);
    for (title, p) in [
        ("low", Priority::Low),
        ("urgent", Priority::Urgent),
        ("none", Priority::None),
    ] {
        let mut nt = new_task(title);
        nt.priority = p;
        ops::add(&store, &h, nt).unwrap();
    }
    ops::done(&store, &h, "3", false).unwrap();

    let by_priority = Filter {
        sort: Sort::Priority,
        ..Filter::default()
    };
    let titles: Vec<String> = ops::list(&store, &h, &by_priority)
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["urgent", "low", "none"]);

    let done_only = Filter {
        state: Some(State::Done),
        ..Filter::default()
    };
    let done: Vec<u64> = ops::list(&store, &h, &done_only)
        .unwrap()
        .iter()
        .map(|t| t.num)
        .collect();
    assert_eq!(done, [3]);
}

// ---- review fixes (0.1.0) ----

#[test]
fn only_the_holder_changes_the_state_of_a_leased_task() {
    let dir = TempDir::new("ops-state-guard");
    let store = Store::new(dir.path());
    add(&store, "x");
    let a = ctx("agent:a", "na", 0);
    let b = ctx("agent:b", "nb", 1);
    ops::start(&store, &a, "1", 10).unwrap();
    let to_todo = Changes {
        state: Some(State::Todo),
        ..Changes::default()
    };

    // Another worker can't silently reset the holder's progress…
    assert!(is_conflict(ops::update(&store, &b, "1", to_todo.clone())));
    assert_eq!(ops::show(&store, "1").unwrap().task.state, State::Doing);
    // …but may still edit metadata (priority, labels) collaboratively…
    let reprioritize = Changes {
        priority: Some(Priority::High),
        label_add: vec!["review".into()],
        ..Changes::default()
    };
    assert_eq!(
        ops::update(&store, &b, "1", reprioritize).unwrap().priority,
        Priority::High
    );
    // …and --force overrides; the holder itself is never blocked.
    let forced = Changes {
        force: true,
        ..to_todo.clone()
    };
    assert_eq!(
        ops::update(&store, &b, "1", forced).unwrap().state,
        State::Todo
    );
    ops::start(&store, &a, "1", 10).unwrap();
    assert_eq!(
        ops::update(&store, &a, "1", to_todo).unwrap().state,
        State::Todo
    );
}

#[test]
fn a_forced_state_change_leaves_the_holders_lease_intact() {
    // --force overrides the holder's say over the *state*; it doesn't take the lease.
    let dir = TempDir::new("ops-force-keeps-lease");
    let store = Store::new(dir.path());
    add(&store, "x");
    let a = ctx("agent:a", "na", 0);
    let b = ctx("agent:b", "nb", 1);
    ops::lease(&store, &a, "1", 10).unwrap();

    let forced = ops::update(
        &store,
        &b,
        "1",
        Changes {
            state: Some(State::Doing),
            force: true,
            ..Changes::default()
        },
    )
    .unwrap();
    assert_eq!(forced.state, State::Doing);
    let lease = forced.lease.expect("a still holds the lease");
    assert!(lease.is_held_by("agent:a", "na"));
    assert!(lease.is_active(b.now_ms));
    assert!(
        is_conflict(ops::lease(&store, &b, "1", 10)),
        "b still can't take it"
    );
}

#[test]
fn ctx_defaults_to_a_private_identity_and_agents_must_name_their_node() {
    let local = Ctx::new(None, None, 0).unwrap();
    assert_eq!(
        (local.actor.as_str(), local.node.as_str()),
        ("human:local", "local")
    );
    let named = Ctx::new(Some(" agent:claude ".into()), Some(" win-1 ".into()), 7).unwrap();
    assert_eq!(
        (named.actor.as_str(), named.node.as_str(), named.now_ms),
        ("agent:claude", "win-1", 7)
    );

    let err = Ctx::new(Some("agent:claude".into()), None, 0).unwrap_err();
    assert!(matches!(err, Error::Usage(_)), "{err:?}");
    assert!(err.to_string().contains("HIPPO_NODE"), "{err}");
    assert!(matches!(
        Ctx::new(Some("  ".into()), None, 0),
        Err(Error::Usage(_))
    ));
    assert!(matches!(
        Ctx::new(None, Some(" ".into()), 0),
        Err(Error::Usage(_))
    ));
}

#[test]
fn a_blank_description_is_refused_by_add_and_update_alike() {
    let dir = TempDir::new("ops-blank-body");
    let store = Store::new(dir.path());
    let mut nt = new_task("x");
    nt.body = Some("   ".into());
    assert!(matches!(
        ops::add(&store, &human(0), nt),
        Err(Error::Usage(_))
    ));
    add(&store, "y");
    let blank = Changes {
        body: Some(" ".into()),
        ..Changes::default()
    };
    assert!(matches!(
        ops::update(&store, &human(0), "1", blank),
        Err(Error::Usage(_))
    ));
}
