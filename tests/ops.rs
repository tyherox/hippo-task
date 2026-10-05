//! The commands' rules (ops.rs), through the library API with a controlled
//! clock — so lease expiry can be tested without sleeping.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::TempDir;
use hippo_task::error::Error;
use hippo_task::media::Anchor;
use hippo_task::model::{Event, EventKind, Priority, RelType, Relation, State, Task};
use hippo_task::ops::{self, Changes, Ctx, Filter, NewTask, ReclaimTarget, Sort};
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
        fields: vec![],
    }
}

fn add(store: &Store, title: &str) -> Task {
    ops::add(store, &human(0), new_task(title)).unwrap()
}

fn is_conflict<T: std::fmt::Debug>(r: Result<T, Error>) -> bool {
    matches!(r, Err(Error::Conflict(_)))
}

/// A timed lease as 0.1.x wrote it. 0.2.0 only writes claims (ADR-003), but
/// existing ledgers still hold leases, and their rules still apply.
fn legacy_lease(store: &Store, who: &Ctx, id: &str, minutes: i64) {
    let task = ops::show(store, id).unwrap().task.id;
    let mut tx = store.begin().unwrap();
    let ts = tx.next_ts(who.now_ms);
    let lease = Event {
        eid: format!("legacy-lease-{ts}"),
        task,
        ts,
        actor: who.actor.clone(),
        node: who.node.clone(),
        kind: EventKind::Lease {
            holder: who.actor.clone(),
            expires_ms: ts + minutes * MIN,
        },
    };
    tx.commit(vec![lease]).unwrap();
}

const A_MONTH: i64 = 30 * 24 * 60 * MIN;

// ---- holding a task over time ----

// test-weaken-ok: ADR-003 (accepted by the maintainer) retires timed leases — `renewing_extends_your_own_lease` and the `lease --minutes` range checks tested a feature that no longer exists; legacy leases keep their expiry tests below.
#[test]
fn a_claim_holds_until_it_is_released() {
    // ADR-003: no timer — a claim holds until its worker gives it back.
    let dir = TempDir::new("ops-claim");
    let store = Store::new(dir.path());
    add(&store, "Ship auth");
    let claimed = ops::start(&store, &ctx("agent:a", "na", 1_000), "1").unwrap();
    let l = claimed.lease.clone().expect("a holds it");
    assert_eq!(l.expires_ms, None);
    assert!(l.since_ms < l.last_seen_ms, "claimed, then set to doing");
    assert_eq!(
        l.last_seen_ms, claimed.updated_ms,
        "starting is a's latest sign of life"
    );

    let b = ctx("agent:b", "nb", 1_000 + A_MONTH);
    assert!(
        is_conflict(ops::start(&store, &b, "1")),
        "a quiet claim still holds"
    );
    ops::release(&store, &ctx("agent:a", "na", 1_000 + A_MONTH), "1").unwrap();
    let taken = ops::start(&store, &b, "1").unwrap();
    assert!(taken
        .lease
        .expect("b holds it now")
        .is_held_by("agent:b", "nb"));
}

#[test]
fn a_legacy_lease_blocks_others_until_it_expires() {
    // Ledgers written by 0.1.x hold timed leases; those still expire as before.
    let dir = TempDir::new("ops-expiry");
    let store = Store::new(dir.path());
    let t = add(&store, "Ship auth");

    legacy_lease(&store, &ctx("agent:a", "na", 1_000), "1", 1);
    assert!(is_conflict(ops::start(
        &store,
        &ctx("agent:b", "nb", 1_000 + 30_000),
        "1"
    )));
    let taken = ops::start(&store, &ctx("agent:b", "nb", 1_000 + MIN + 1), "1").unwrap();
    let lease = taken.lease.expect("b holds it now");
    assert_eq!(
        (lease.holder.as_str(), lease.node.as_str()),
        ("agent:b", "nb")
    );
    assert_eq!(taken.id, t.id);
}

#[test]
fn mine_includes_a_claim_however_old() {
    let dir = TempDir::new("ops-mine-claim");
    let store = Store::new(dir.path());
    add(&store, "one");
    ops::start(&store, &ctx("agent:a", "na", 0), "1").unwrap();
    let mine = Filter {
        mine: true,
        ..Filter::default()
    };
    let later = ops::list(&store, &ctx("agent:a", "na", A_MONTH), &mine).unwrap();
    assert_eq!(later.iter().map(|t| t.num).collect::<Vec<_>>(), [1]);
}

#[test]
fn mine_lists_only_active_leases_held_by_this_worker() {
    let dir = TempDir::new("ops-mine");
    let store = Store::new(dir.path());
    add(&store, "one");
    add(&store, "two");
    legacy_lease(&store, &ctx("agent:a", "na", 0), "1", 1);
    legacy_lease(&store, &ctx("agent:a", "other-window", 0), "2", 1);

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
    ops::start(&store, &ctx("agent:a", "na", 0), "1").unwrap();

    let by_b = ops::release(&store, &ctx("agent:b", "nb", 1), "1").unwrap();
    assert!(!by_b.released);
    assert!(by_b.task.lease.is_some(), "b can't release a's lease");

    let by_a = ops::release(&store, &ctx("agent:a", "na", 2), "1").unwrap();
    assert!(by_a.released);
    assert!(by_a.task.lease.is_none());
}

/// Event kinds in a task's history, with whether each one changed anything.
fn history(store: &Store, id: &str) -> Vec<(&'static str, bool)> {
    ops::show(store, id)
        .unwrap()
        .history
        .iter()
        .map(|e| (e.event.kind.name(), e.applied))
        .collect()
}

#[test]
fn releasing_a_task_you_started_puts_it_back_in_the_queue() {
    // ADR-002: `release` undoes `start` — the lease *and* the `doing`.
    let dir = TempDir::new("ops-release-to-todo");
    let store = Store::new(dir.path());
    add(&store, "x");
    let a = ctx("agent:a", "na", 0);
    ops::start(&store, &a, "1").unwrap();

    let r = ops::release(&store, &a, "1").unwrap();
    assert!(r.released);
    assert!(r.task.lease.is_none());
    assert_eq!(r.task.state, State::Todo, "released work goes back to todo");
    // Undone in reverse: the holder resets the state while still holding the
    // lease (the change is theirs to make), then lets go. Both take effect.
    assert_eq!(
        history(&store, "1"),
        [
            ("create", true),
            ("claim", true),
            ("set-state", true),
            ("set-state", true),
            ("release", true),
        ]
    );
}

#[test]
fn releasing_a_task_you_only_leased_leaves_its_state_alone() {
    let dir = TempDir::new("ops-release-leased-only");
    let store = Store::new(dir.path());
    add(&store, "x");
    let a = ctx("agent:a", "na", 0);
    legacy_lease(&store, &a, "1", 10);

    let r = ops::release(&store, &a, "1").unwrap();
    assert!(r.released);
    assert_eq!(r.task.state, State::Todo);
    assert_eq!(
        history(&store, "1"),
        [("create", true), ("lease", true), ("release", true)],
        "nothing to reset, so no state event is recorded"
    );
}

#[test]
fn someone_elses_release_leaves_a_started_task_alone() {
    let dir = TempDir::new("ops-release-not-yours");
    let store = Store::new(dir.path());
    add(&store, "x");
    ops::start(&store, &ctx("agent:a", "na", 0), "1").unwrap();

    let by_b = ops::release(&store, &ctx("agent:b", "nb", 1), "1").unwrap();
    assert!(!by_b.released);
    assert_eq!(by_b.task.state, State::Doing, "b can't hand back a's work");
    assert!(by_b
        .task
        .lease
        .as_ref()
        .is_some_and(|l| l.is_held_by("agent:a", "na")));
}

// ---- handing work back: whoever knows (ADR-003) ----

fn nums(tasks: &[Task]) -> Vec<u64> {
    tasks.iter().map(|t| t.num).collect()
}

fn reclaimed(back: &[ops::Reclaimed]) -> Vec<u64> {
    back.iter().map(|r| r.task.num).collect()
}

#[test]
fn release_all_gives_back_everything_this_worker_holds() {
    // For a session-end hook or a workflow's teardown.
    let dir = TempDir::new("ops-release-all");
    let store = Store::new(dir.path());
    for title in ["mine", "also mine", "my other window's", "someone else's"] {
        add(&store, title);
    }
    let a = ctx("agent:a", "na", 0);
    ops::start(&store, &a, "1").unwrap();
    ops::start(&store, &a, "2").unwrap();
    ops::start(&store, &ctx("agent:a", "other-window", 0), "3").unwrap();
    ops::start(&store, &ctx("agent:b", "nb", 0), "4").unwrap();

    let released = ops::release_all(&store, &a).unwrap();
    assert_eq!(nums(&released), [1, 2]);
    assert!(released
        .iter()
        .all(|t| t.state == State::Todo && t.lease.is_none()));
    for id in ["3", "4"] {
        assert!(ops::show(&store, id).unwrap().task.lease.is_some(), "#{id}");
    }
    assert!(
        ops::release_all(&store, &a).unwrap().is_empty(),
        "nothing left"
    );
}

#[test]
fn a_person_reclaims_a_crashed_workers_task() {
    let dir = TempDir::new("ops-reclaim");
    let store = Store::new(dir.path());
    add(&store, "x");
    ops::start(&store, &ctx("agent:a", "na", 0), "1").unwrap();

    let target = ReclaimTarget::Task("1".into());
    let back = ops::reclaim(&store, &human(MIN), &target, Some("window closed"), false).unwrap();
    assert_eq!(reclaimed(&back), [1]);
    assert!(
        back[0].from.is_held_by("agent:a", "na"),
        "it says whose claim it was"
    );
    assert_eq!(back[0].task.state, State::Todo, "back in the queue");
    assert!(back[0].task.lease.is_none());
    let history = history(&store, "1");
    assert_eq!(
        history[history.len() - 2..],
        [("reclaim", true), ("set-state", true)]
    );
    let detail = ops::show(&store, "1").unwrap();
    let reclaim = &detail.history[history.len() - 2].event;
    assert!(
        matches!(&reclaim.kind, EventKind::Reclaim { holder, node, reason }
            if holder == "agent:a" && node == "na" && reason.as_deref() == Some("window closed")),
        "the history says whose claim was taken back, and why: {reclaim:?}"
    );
    ops::start(&store, &ctx("agent:b", "nb", 2 * MIN), "1").unwrap();
}

#[test]
fn an_agent_needs_force_to_reclaim() {
    // --force is for the process that launched the worker (AGENTS.md).
    let dir = TempDir::new("ops-reclaim-agent");
    let store = Store::new(dir.path());
    add(&store, "x");
    ops::start(&store, &ctx("agent:a", "na", 0), "1").unwrap();
    let orchestrator = ctx("agent:a", "orchestrator", 1);
    let target = ReclaimTarget::Task("1".into());

    assert!(is_conflict(ops::reclaim(
        &store,
        &orchestrator,
        &target,
        None,
        false
    )));
    let back = ops::reclaim(&store, &orchestrator, &target, None, true).unwrap();
    assert_eq!(reclaimed(&back), [1]);
}

#[test]
fn reclaiming_a_task_nobody_holds_changes_nothing() {
    let dir = TempDir::new("ops-reclaim-free");
    let store = Store::new(dir.path());
    add(&store, "x");
    let before = store.read().unwrap().events.len();
    let target = ReclaimTarget::Task("1".into());
    let back = ops::reclaim(&store, &human(0), &target, None, false).unwrap();
    assert!(back.is_empty());
    assert_eq!(
        store.read().unwrap().events.len(),
        before,
        "nothing to record"
    );
}

#[test]
fn reclaim_from_a_node_takes_back_everything_it_holds() {
    let dir = TempDir::new("ops-reclaim-node");
    let store = Store::new(dir.path());
    for title in ["a", "b", "c"] {
        add(&store, title);
    }
    ops::start(&store, &ctx("agent:a", "wf-1", 0), "1").unwrap();
    ops::start(&store, &ctx("agent:a", "wf-1", 0), "2").unwrap();
    ops::start(&store, &ctx("agent:a", "wf-2", 0), "3").unwrap();

    let target = ReclaimTarget::Node("wf-1".into());
    let back = ops::reclaim(&store, &human(MIN), &target, Some("worker failed"), false).unwrap();
    assert_eq!(reclaimed(&back), [1, 2]);
    assert!(ops::show(&store, "3").unwrap().task.lease.is_some());
}

#[test]
fn held_lists_who_holds_what_and_when_each_was_last_seen() {
    let dir = TempDir::new("ops-held");
    let store = Store::new(dir.path());
    for title in ["busy", "quiet", "free"] {
        add(&store, title);
    }
    let a = |now| ctx("agent:a", "na", now);
    ops::start(&store, &a(0), "1").unwrap();
    ops::start(&store, &ctx("agent:b", "nb", 0), "2").unwrap();
    let noted = ops::note(&store, &a(5 * MIN), "1", "halfway").unwrap();

    let held = Filter {
        held: true,
        ..Filter::default()
    };
    let tasks = ops::list(&store, &human(10 * MIN), &held).unwrap();
    assert_eq!(nums(&tasks), [1, 2]);
    let seen = |t: &Task| t.lease.as_ref().map(|l| (l.since_ms, l.last_seen_ms));
    let busy = seen(&tasks[0]).expect("held");
    assert_eq!(busy.1, noted.updated_ms, "a's note is a sign of life");
    let quiet = seen(&tasks[1]).expect("held");
    assert_eq!(
        quiet.1, tasks[1].updated_ms,
        "b hasn't been seen since it started #2"
    );
    assert!(quiet.1 < busy.1);
}

// ---- duplicates: find before filing, mark when found (ADR-004) ----

fn search(store: &Store, terms: &[&str]) -> Vec<u64> {
    let filter = Filter {
        search: terms.iter().map(|t| t.to_string()).collect(),
        ..Filter::default()
    };
    nums(&ops::list(store, &human(0), &filter).unwrap())
}

#[test]
fn search_finds_text_in_titles_or_descriptions_in_any_state() {
    let dir = TempDir::new("ops-search");
    let store = Store::new(dir.path());
    add(&store, "token_refresh races after an hour");
    let described = NewTask {
        body: Some("Suspect the Token_Refresh lock".into()),
        ..new_task("Users get a 401")
    };
    ops::add(&store, &human(0), described).unwrap();
    add(&store, "Write docs");
    ops::done(&store, &human(0), "2", false).unwrap(); // the original may be done already

    assert_eq!(
        search(&store, &["TOKEN_REFRESH"]),
        [1, 2],
        "any case; title or description; any state"
    );
    assert_eq!(
        search(&store, &["token_refresh", "hour"]),
        [1],
        "every term must match"
    );
    assert!(search(&store, &["nothing like this"]).is_empty());
}

#[test]
fn a_blank_search_is_a_usage_error() {
    let dir = TempDir::new("ops-search-blank");
    let store = Store::new(dir.path());
    let filter = Filter {
        search: vec!["  ".into()],
        ..Filter::default()
    };
    assert!(matches!(
        ops::list(&store, &human(0), &filter),
        Err(Error::Usage(_))
    ));
}

fn duplicate_of(original: &str) -> Changes {
    Changes {
        duplicate_of: Some(original.into()),
        ..Changes::default()
    }
}

#[test]
fn marking_a_duplicate_links_it_to_the_original_and_cancels_it() {
    let dir = TempDir::new("ops-dup");
    let store = Store::new(dir.path());
    let original = add(&store, "token_refresh races after an hour");
    add(&store, "token_refresh race, filed again");

    let dup = ops::update(&store, &human(0), "2", duplicate_of("1")).unwrap();
    assert_eq!(
        dup.state,
        State::Cancelled,
        "a duplicate isn't finished work"
    );
    assert_eq!(
        dup.relations,
        [Relation {
            rel: RelType::DuplicateOf,
            task: original.id.clone()
        }]
    );
    assert!(!dup.blocked, "a duplicate link never blocks");
    assert_eq!(
        history(&store, "2")[1..],
        [("relate", true), ("set-state", true)]
    );
}

#[test]
fn a_duplicate_closed_as_done_is_corrected_to_cancelled() {
    // Before 0.3.0, agents closed duplicates as `done`; marking one fixes the record.
    let dir = TempDir::new("ops-dup-done");
    let store = Store::new(dir.path());
    add(&store, "original");
    add(&store, "filed again");
    ops::done(&store, &human(0), "2", false).unwrap();
    let dup = ops::update(&store, &human(1), "2", duplicate_of("1")).unwrap();
    assert_eq!(dup.state, State::Cancelled);
}

#[test]
fn duplicate_of_rules() {
    let dir = TempDir::new("ops-dup-rules");
    let store = Store::new(dir.path());
    add(&store, "one");
    add(&store, "two");
    let h = human(0);
    assert!(
        matches!(
            ops::update(&store, &h, "1", duplicate_of("1")),
            Err(Error::Usage(_))
        ),
        "a task isn't its own duplicate"
    );
    assert!(matches!(
        ops::update(&store, &h, "1", duplicate_of("99")),
        Err(Error::NotFound(_))
    ));
    let with_state = Changes {
        state: Some(State::Todo),
        ..duplicate_of("2")
    };
    assert!(
        matches!(
            ops::update(&store, &h, "1", with_state),
            Err(Error::Usage(_))
        ),
        "--duplicate-of already sets the state"
    );
}

#[test]
fn only_the_holder_marks_a_held_task_as_a_duplicate() {
    let dir = TempDir::new("ops-dup-held");
    let store = Store::new(dir.path());
    add(&store, "original");
    add(&store, "filed again");
    ops::start(&store, &ctx("agent:a", "na", 0), "2").unwrap();

    let b = ctx("agent:b", "nb", 1);
    assert!(is_conflict(ops::update(&store, &b, "2", duplicate_of("1"))));
    let by_a = ops::update(&store, &ctx("agent:a", "na", 2), "2", duplicate_of("1")).unwrap();
    assert_eq!(by_a.state, State::Cancelled);
    assert!(by_a.lease.is_none(), "closing clears the claim");
}

#[test]
fn ready_is_open_unblocked_unheld_work_including_abandoned_tasks() {
    // ADR-002: the pick query. A `doing` task whose 0.1.x lease ran out is ready
    // again — the fold can't change its state, but `--ready` sees it. A claim
    // (ADR-003) never runs out: it stays held until given back or reclaimed.
    let dir = TempDir::new("ops-ready");
    let store = Store::new(dir.path());
    for title in [
        "free",
        "leased",
        "abandoned",
        "blocked",
        "closed",
        "in progress",
    ] {
        add(&store, title);
    }
    let a = ctx("agent:a", "na", 0);
    let b = ctx("agent:b", "nb", 0);
    legacy_lease(&store, &a, "2", 60); // todo, but someone holds it
    legacy_lease(&store, &b, "3", 10); // b started #3 under 0.1.x and "crashed":
    let doing = Changes {
        state: Some(State::Doing),
        ..Changes::default()
    };
    ops::update(&store, &b, "3", doing).unwrap(); // its lease runs out at 10 min
    let blocked_by_1 = Changes {
        block: vec!["1".into()],
        ..Changes::default()
    };
    ops::update(&store, &human(0), "4", blocked_by_1).unwrap();
    ops::done(&store, &human(0), "5", false).unwrap();
    ops::start(&store, &a, "6").unwrap(); // a is still on it, however long it takes

    let ready = |now_ms| {
        let filter = Filter {
            ready: true,
            ..Filter::default()
        };
        let tasks = ops::list(&store, &human(now_ms), &filter).unwrap();
        tasks.iter().map(|t| t.num).collect::<Vec<_>>()
    };
    assert_eq!(
        ready(5 * MIN),
        [1],
        "only the free task, while b's lease holds"
    );
    assert_eq!(
        ready(20 * MIN),
        [1, 3],
        "b's lease ran out: its doing task is ready for someone else"
    );
    assert_eq!(
        ready(A_MONTH),
        [1, 2, 3],
        "#2's old lease has run out too; a's claim on #6 never does"
    );
}

#[test]
fn a_refused_start_records_only_the_attempt() {
    let dir = TempDir::new("ops-start");
    let store = Store::new(dir.path());
    add(&store, "x");
    ops::start(&store, &ctx("agent:a", "na", 0), "1").unwrap();
    let before = store.read().unwrap().events.len();

    assert!(is_conflict(ops::start(
        &store,
        &ctx("agent:b", "nb", 1),
        "1"
    )));
    let after = store.read().unwrap().events;
    assert_eq!(
        after.len(),
        before + 1,
        "the rejected claim is audited, nothing else"
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
    ops::start(&store, &ctx("agent:a", "na", 0), "1").unwrap();
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
    ops::start(&store, &a, "1").unwrap();
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
        is_conflict(ops::start(&store, &ctx("agent:b", "nb", 1), "1")),
        "closed tasks can't be claimed"
    );
}

#[test]
fn an_expired_lease_does_not_protect_the_task() {
    let dir = TempDir::new("ops-expired-close");
    let store = Store::new(dir.path());
    add(&store, "x");
    legacy_lease(&store, &ctx("agent:a", "na", 0), "1", 1);
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
        ops::describe(&store, &h, "1", "  ", &Anchor::Cwd, None),
        Err(Error::Usage(_))
    ));
    assert!(matches!(
        ops::reclaim(
            &store,
            &h,
            &ReclaimTarget::Task("1".into()),
            Some("  "),
            false
        ),
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
    ops::start(&store, &a, "1").unwrap();
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
    ops::start(&store, &a, "1").unwrap();
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
    legacy_lease(&store, &a, "1", 10);

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
        is_conflict(ops::start(&store, &b, "1")),
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
