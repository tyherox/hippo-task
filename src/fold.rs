//! The projection: fold append-only events into current task state.
//!
//! This is the heart of the design — **concurrency and audit both live here.**
//! `fold` is a *pure function*: the same events give the same tasks, in any
//! input order. That is what makes the ledger safe to share — two agents (or,
//! later, two machines) holding the same events always agree on the state.
//! The property test at the bottom of this file checks exactly that.
//!
//! The rules, all in this file:
//! - **Order.** Events apply in `(ts, node, eid)` order — a total order every
//!   reader computes identically, whatever order the file lists them in.
//! - **Scalars** (title, state, priority, assignee, body): last writer wins.
//! - **Collections** (labels, relations): set operations, so two agents adding
//!   different labels at the same time both survive.
//! - **Idempotent.** An event that changes nothing is still in the ledger
//!   (audit) but doesn't bump `seq`/`updated_ms`; it's listed in
//!   [`Projection::noops`] so the history can say "(no change)" / "(rejected)".
//! - **Holds.** A claim (0.2.0: no timer) or a lease (0.1.x: timed) is granted
//!   only on an open task whose hold is free, expired, or already this worker's
//!   (actor + node). A rejected attempt is a no-op. A reclaim takes back only
//!   the hold it names (ADR-003).
//! - **Signs of life.** Any event the holder writes on its task refreshes the
//!   hold's `last_seen_ms` — derived, like `blocked`, so it's never a "change".
//! - **Closed tasks hold nothing.** Entering done/cancelled clears the hold.
//! - **Blocked is derived**, never stored: a task is blocked while any task it
//!   is blocked-by is still open.
//!
//! "Physics vs etiquette": these rules hold for *any* ledger, including ones
//! written by other tools. Politeness rules ("only the holder may close a
//! leased task") live in `ops.rs`, because a merge must never refuse a fact.

use crate::model::{Event, EventKind, Lease, RelType, Relation, State, Task};
use std::collections::{BTreeMap, BTreeSet};

/// The result of folding a ledger.
#[derive(Debug, Default, PartialEq)]
pub struct Projection {
    /// Current state of every task, keyed by task id.
    pub tasks: BTreeMap<String, Task>,
    /// `eid`s of events that changed nothing: repeats, rejected leases, a
    /// release by a non-holder, events for tasks that don't exist (yet).
    pub noops: BTreeSet<String>,
}

/// What one event did to its task. The bookkeeping for all three cases lives
/// in one place (`fold`), so no rule can forget to bump `seq`.
enum Effect {
    /// Nothing changed.
    None,
    /// Activity without a content change (a note): bumps `updated_ms`, not `seq`.
    Activity,
    /// Content changed: bumps `updated_ms` and `seq`.
    Changed,
}

/// Events sorted into the one order every reader agrees on: `(ts, node, eid)`.
/// `fold` and the history view both use this, so they can never disagree.
pub fn in_order<'a>(events: impl IntoIterator<Item = &'a Event>) -> Vec<&'a Event> {
    let mut sorted: Vec<&Event> = events.into_iter().collect();
    sorted.sort_by(|a, b| {
        a.ts.cmp(&b.ts)
            .then_with(|| a.node.cmp(&b.node))
            .then_with(|| a.eid.cmp(&b.eid))
    });
    sorted
}

/// Fold events into the current set of tasks.
///
/// Rust note: `impl IntoIterator<Item = &'a Event>` accepts a slice, a `&Vec`,
/// or a chain of iterators — `ops::start` folds "the ledger plus one more
/// event" to preview a lease decision without copying anything.
pub fn fold<'a>(events: impl IntoIterator<Item = &'a Event>) -> Projection {
    let mut out = Projection::default();
    let mut next_num: u64 = 1;

    for ev in in_order(events) {
        // Only a Create introduces a task, and the first Create for an id wins.
        if let EventKind::Create {
            title,
            priority,
            body,
            assignee,
        } = &ev.kind
        {
            if out.tasks.contains_key(&ev.task) {
                out.noops.insert(ev.eid.clone());
            } else {
                let task = Task {
                    id: ev.task.clone(),
                    num: next_num,
                    title: title.clone(),
                    body: body.clone(),
                    state: State::Todo,
                    priority: *priority,
                    assignee: assignee.clone(),
                    labels: BTreeSet::new(),
                    relations: Vec::new(),
                    lease: None,
                    created_ms: ev.ts,
                    updated_ms: ev.ts,
                    seq: 1,
                    blocked: false,
                };
                out.tasks.insert(ev.task.clone(), task);
                next_num += 1;
            }
            continue;
        }

        // Every other event needs its task to exist (in fold order).
        let Some(task) = out.tasks.get_mut(&ev.task) else {
            out.noops.insert(ev.eid.clone());
            continue;
        };
        match apply(task, ev) {
            Effect::Changed => {
                task.updated_ms = ev.ts;
                task.seq += 1;
            }
            Effect::Activity => task.updated_ms = ev.ts,
            Effect::None => {
                out.noops.insert(ev.eid.clone());
            }
        }
        // A sign of life (ADR-003): any event the holder writes on its task —
        // applied or not — says when it was last seen. It's derived, like
        // `blocked`, so it never counts as a change.
        if let Some(l) = task.lease.as_mut() {
            if l.is_held_by(&ev.actor, &ev.node) {
                l.last_seen_ms = ev.ts;
            }
        }
    }

    derive_blocked(&mut out.tasks);
    out
}

/// Apply one (non-Create) event to its task, and say what it did.
fn apply(t: &mut Task, ev: &Event) -> Effect {
    match &ev.kind {
        // Handled in `fold` — a Create never reaches here. Listed so this match
        // stays exhaustive: add an event kind and the compiler sends you here.
        EventKind::Create { .. } => Effect::None,

        // --- Scalars: last writer wins; setting the same value changes nothing.
        EventKind::SetTitle { title } => changed(set(&mut t.title, title.clone())),
        EventKind::SetPriority { priority } => changed(set(&mut t.priority, *priority)),
        EventKind::SetAssignee { assignee } => changed(set(&mut t.assignee, assignee.clone())),
        EventKind::SetBody { body } => changed(set(&mut t.body, Some(body.clone()))),
        EventKind::SetState { state } => {
            let state_changed = set(&mut t.state, *state);
            // Invariant: a closed task holds no lease.
            let lease_cleared = t.state.is_closed() && t.lease.take().is_some();
            changed(state_changed || lease_cleared)
        }

        // --- Collections: set operations, so concurrent edits merge.
        EventKind::LabelAdd { label } => changed(t.labels.insert(label.clone())),
        EventKind::LabelRemove { label } => changed(t.labels.remove(label)),
        EventKind::Relate { rel, task } => {
            let r = Relation {
                rel: *rel,
                task: task.clone(),
            };
            let is_new = !t.relations.contains(&r);
            if is_new {
                t.relations.push(r);
            }
            changed(is_new)
        }
        EventKind::Unrelate { rel, task } => {
            let before = t.relations.len();
            t.relations.retain(|r| !(r.rel == *rel && r.task == *task));
            changed(t.relations.len() != before)
        }

        // --- Holds: the swarm-coordination rule. A claim is a lease with no timer.
        EventKind::Lease { holder, expires_ms } => grant(t, ev, holder, Some(*expires_ms)),
        EventKind::Claim { holder } => grant(t, ev, holder, None),
        // Take back only the hold this names: if that worker already let go —
        // and someone newer may hold the task by now — it changes nothing.
        EventKind::Reclaim { holder, node, .. } => {
            let named = t.lease.as_ref().is_some_and(|l| l.is_held_by(holder, node));
            if named {
                t.lease = None;
            }
            changed(named)
        }
        // You can only give back a lease *you* (this actor on this node) hold.
        EventKind::Release => {
            let mine = t
                .lease
                .as_ref()
                .is_some_and(|l| l.is_held_by(&ev.actor, &ev.node));
            if mine {
                t.lease = None;
            }
            changed(mine)
        }
        EventKind::Complete => {
            let state_changed = set(&mut t.state, State::Done);
            let lease_cleared = t.lease.take().is_some();
            changed(state_changed || lease_cleared)
        }

        // --- A note is activity, not a content change.
        EventKind::Note { .. } => Effect::Activity,
    }
}

/// Grant a hold — a timed lease (`expires_ms: Some`) or a claim (`None`) — if
/// the task is open and its current hold is free, expired, or this worker's own.
///
/// Rust note: `Option::is_none_or` reads "no hold, or a hold that…" — a
/// one-line way to say "free" without a `match`.
fn grant(t: &mut Task, ev: &Event, holder: &str, expires_ms: Option<i64>) -> Effect {
    let current = t.lease.as_ref();
    let mine = current.is_some_and(|l| l.is_held_by(holder, &ev.node));
    let free = current.is_none_or(|l| !l.is_active(ev.ts)) || mine;
    if t.state.is_closed() || !free {
        return Effect::None; // rejected — the attempt stays in the ledger
    }
    let unbroken = mine && current.is_some_and(|l| l.is_active(ev.ts));
    if unbroken && current.is_some_and(|l| l.expires_ms == expires_ms) {
        return Effect::None; // already held on exactly these terms
    }
    // Same worker, no gap: the hold keeps its start. Anyone else starts afresh.
    let since_ms = match current {
        Some(l) if unbroken => l.since_ms,
        _ => ev.ts,
    };
    t.lease = Some(Lease {
        holder: holder.to_string(),
        node: ev.node.clone(),
        expires_ms,
        since_ms,
        last_seen_ms: ev.ts,
    });
    Effect::Changed
}

/// Last-writer-wins assignment that reports whether anything changed.
///
/// Rust note: `T: PartialEq` is a *trait bound* — this one function works for
/// titles (`String`), states, priorities, `Option`s… anything comparable.
fn set<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        false
    } else {
        *slot = value;
        true
    }
}

fn changed(did_change: bool) -> Effect {
    if did_change {
        Effect::Changed
    } else {
        Effect::None
    }
}

/// Derived, not stored: an open task is blocked iff some blocked-by target is
/// an *open* task. A closed blocker (done or cancelled) no longer blocks; a
/// missing one never did; a task can't block itself; and a closed task is
/// never blocked — nothing is left to wait for.
fn derive_blocked(tasks: &mut BTreeMap<String, Task>) {
    let open: BTreeSet<String> = tasks
        .values()
        .filter(|t| !t.state.is_closed())
        .map(|t| t.id.clone())
        .collect();
    for t in tasks.values_mut() {
        t.blocked = !t.state.is_closed()
            && t.relations
                .iter()
                .any(|r| r.rel == RelType::BlockedBy && r.task != t.id && open.contains(&r.task));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Priority;

    // ---- a tiny DSL so each test reads like the scenario it checks ----

    const HUMAN: (&str, &str) = ("human:local", "n0");
    const A: (&str, &str) = ("agent:a", "na");
    const B: (&str, &str) = ("agent:b", "nb");

    fn ev(eid: &str, task: &str, ts: i64, who: (&str, &str), kind: EventKind) -> Event {
        Event {
            eid: eid.into(),
            task: task.into(),
            ts,
            actor: who.0.into(),
            node: who.1.into(),
            kind,
        }
    }

    fn create(eid: &str, task: &str, ts: i64) -> Event {
        let kind = EventKind::Create {
            title: format!("title of {task}"),
            priority: Priority::None,
            body: None,
            assignee: None,
        };
        ev(eid, task, ts, HUMAN, kind)
    }

    fn lease(eid: &str, task: &str, ts: i64, who: (&str, &str), expires_ms: i64) -> Event {
        let kind = EventKind::Lease {
            holder: who.0.into(),
            expires_ms,
        };
        ev(eid, task, ts, who, kind)
    }

    fn state(eid: &str, task: &str, ts: i64, s: State) -> Event {
        ev(eid, task, ts, HUMAN, EventKind::SetState { state: s })
    }

    fn block(eid: &str, task: &str, ts: i64, on: &str) -> Event {
        let kind = EventKind::Relate {
            rel: RelType::BlockedBy,
            task: on.into(),
        };
        ev(eid, task, ts, HUMAN, kind)
    }

    fn claim(eid: &str, task: &str, ts: i64, who: (&str, &str)) -> Event {
        let kind = EventKind::Claim {
            holder: who.0.into(),
        };
        ev(eid, task, ts, who, kind)
    }

    /// `by` takes back the claim held by `from`.
    fn reclaim(eid: &str, task: &str, ts: i64, by: (&str, &str), from: (&str, &str)) -> Event {
        let kind = EventKind::Reclaim {
            holder: from.0.into(),
            node: from.1.into(),
            reason: None,
        };
        ev(eid, task, ts, by, kind)
    }

    fn task<'a>(p: &'a Projection, id: &str) -> &'a Task {
        p.tasks.get(id).expect("task exists")
    }

    // ---- ordering ----

    #[test]
    fn create_assigns_numbers_in_time_order() {
        let p = fold(&[create("e2", "T2", 20), create("e1", "T1", 10)]);
        assert_eq!(task(&p, "T1").num, 1);
        assert_eq!(task(&p, "T2").num, 2);
        assert_eq!(task(&p, "T1").seq, 1);
        assert_eq!(task(&p, "T1").state, State::Todo);
    }

    #[test]
    fn input_order_does_not_matter() {
        let events = vec![
            create("e1", "T1", 10),
            ev(
                "e2",
                "T1",
                20,
                A,
                EventKind::SetTitle {
                    title: "first".into(),
                },
            ),
            ev(
                "e3",
                "T1",
                30,
                B,
                EventKind::SetTitle {
                    title: "second".into(),
                },
            ),
        ];
        let mut reversed = events.clone();
        reversed.reverse();
        assert_eq!(fold(&events), fold(&reversed));
        assert_eq!(task(&fold(&reversed), "T1").title, "second");
    }

    #[test]
    fn same_timestamp_ties_break_by_node_then_eid() {
        // Same ts: node "nb" sorts after "na", so B's write is applied last and wins.
        let events = [
            create("e0", "T1", 1),
            ev(
                "e9",
                "T1",
                5,
                B,
                EventKind::SetTitle {
                    title: "from b".into(),
                },
            ),
            ev(
                "e1",
                "T1",
                5,
                A,
                EventKind::SetTitle {
                    title: "from a".into(),
                },
            ),
        ];
        assert_eq!(task(&fold(&events), "T1").title, "from b");

        // Same ts and node: the eid decides.
        let events = [
            create("e0", "T1", 1),
            ev(
                "e2",
                "T1",
                5,
                A,
                EventKind::SetTitle {
                    title: "later eid".into(),
                },
            ),
            ev(
                "e1",
                "T1",
                5,
                A,
                EventKind::SetTitle {
                    title: "earlier eid".into(),
                },
            ),
        ];
        assert_eq!(task(&fold(&events), "T1").title, "later eid");
    }

    // ---- scalars, collections, idempotency ----

    #[test]
    fn scalars_are_last_writer_wins() {
        let p = fold(&[
            create("e1", "T1", 1),
            ev(
                "e2",
                "T1",
                2,
                A,
                EventKind::SetPriority {
                    priority: Priority::High,
                },
            ),
            ev(
                "e3",
                "T1",
                3,
                B,
                EventKind::SetPriority {
                    priority: Priority::Low,
                },
            ),
        ]);
        assert_eq!(task(&p, "T1").priority, Priority::Low);
        assert_eq!(task(&p, "T1").seq, 3);
        assert_eq!(task(&p, "T1").updated_ms, 3);
    }

    #[test]
    fn setting_the_same_value_is_a_recorded_no_op() {
        let p = fold(&[
            create("e1", "T1", 1),
            ev(
                "e2",
                "T1",
                2,
                A,
                EventKind::SetState {
                    state: State::Doing,
                },
            ),
            ev(
                "e3",
                "T1",
                3,
                A,
                EventKind::SetState {
                    state: State::Doing,
                },
            ),
            ev("e4", "T1", 4, A, EventKind::LabelAdd { label: "x".into() }),
            ev("e5", "T1", 5, B, EventKind::LabelAdd { label: "x".into() }),
        ]);
        let t = task(&p, "T1");
        assert_eq!(t.seq, 3, "create + doing + first label");
        assert_eq!(t.updated_ms, 4, "no-ops don't touch updated_ms");
        let expected: BTreeSet<String> = ["e3".to_string(), "e5".to_string()].into();
        assert_eq!(p.noops, expected);
    }

    #[test]
    fn concurrent_label_adds_both_survive() {
        // Same millisecond, different agents: a naive last-writer-wins would drop one.
        let p = fold(&[
            create("e1", "T1", 1),
            ev(
                "e2",
                "T1",
                7,
                A,
                EventKind::LabelAdd {
                    label: "frontend".into(),
                },
            ),
            ev(
                "e3",
                "T1",
                7,
                B,
                EventKind::LabelAdd {
                    label: "review".into(),
                },
            ),
        ]);
        let labels: Vec<&str> = task(&p, "T1").labels.iter().map(String::as_str).collect();
        assert_eq!(labels, ["frontend", "review"]);
    }

    #[test]
    fn removing_a_missing_label_is_a_no_op() {
        let p = fold(&[
            create("e1", "T1", 1),
            ev(
                "e2",
                "T1",
                2,
                A,
                EventKind::LabelRemove {
                    label: "nope".into(),
                },
            ),
        ]);
        assert_eq!(task(&p, "T1").seq, 1);
        assert!(p.noops.contains("e2"));
    }

    #[test]
    fn relations_are_a_set() {
        let p = fold(&[
            create("e1", "T1", 1),
            create("e2", "T2", 2),
            block("e3", "T1", 3, "T2"),
            block("e4", "T1", 4, "T2"),
        ]);
        assert_eq!(task(&p, "T1").relations.len(), 1);
        assert!(p.noops.contains("e4"));
    }

    #[test]
    fn a_note_is_activity_not_a_change() {
        let p = fold(&[
            create("e1", "T1", 1),
            ev(
                "e2",
                "T1",
                9,
                A,
                EventKind::Note {
                    text: "left off at X".into(),
                },
            ),
        ]);
        let t = task(&p, "T1");
        assert_eq!(t.seq, 1, "notes don't count as content changes");
        assert_eq!(t.updated_ms, 9, "…but they are activity");
        assert!(p.noops.is_empty(), "a note is never a no-op");
    }

    #[test]
    fn events_for_unknown_tasks_are_recorded_no_ops() {
        let p = fold(&[
            ev("e1", "T9", 1, A, EventKind::LabelAdd { label: "x".into() }),
            create("e2", "T9", 2), // the label came *before* the create in time
        ]);
        assert!(task(&p, "T9").labels.is_empty());
        assert!(p.noops.contains("e1"));
    }

    #[test]
    fn a_second_create_for_the_same_id_is_ignored() {
        let mut dup = create("e2", "T1", 2);
        if let EventKind::Create { title, .. } = &mut dup.kind {
            *title = "impostor".into();
        }
        let p = fold(&[create("e1", "T1", 1), dup]);
        assert_eq!(task(&p, "T1").title, "title of T1");
        assert_eq!(p.tasks.len(), 1);
        assert!(p.noops.contains("e2"));
    }

    // ---- leases ----

    #[test]
    fn lease_is_granted_when_free_and_rejected_when_held() {
        let p = fold(&[
            create("e1", "T1", 1),
            lease("e2", "T1", 10, A, 1_000),
            lease("e3", "T1", 20, B, 2_000),
        ]);
        let l = task(&p, "T1").lease.as_ref().expect("leased");
        assert!(l.is_held_by("agent:a", "na"));
        assert_eq!(l.expires_ms, Some(1_000));
        assert!(
            p.noops.contains("e3"),
            "B's attempt is recorded but rejected"
        );
    }

    #[test]
    fn an_expired_lease_can_be_taken_over() {
        let p = fold(&[
            create("e1", "T1", 1),
            lease("e2", "T1", 10, A, 100),
            lease("e3", "T1", 100, B, 2_000), // exactly at expiry: free
        ]);
        assert!(task(&p, "T1")
            .lease
            .as_ref()
            .expect("leased")
            .is_held_by("agent:b", "nb"));
    }

    #[test]
    fn the_same_worker_renews_its_lease() {
        let p = fold(&[
            create("e1", "T1", 1),
            lease("e2", "T1", 10, A, 100),
            lease("e3", "T1", 50, A, 500),
        ]);
        assert_eq!(
            task(&p, "T1").lease.as_ref().expect("leased").expires_ms,
            Some(500)
        );
    }

    #[test]
    fn the_same_actor_on_another_node_is_another_worker() {
        // Two windows of the same agent must not both "hold" one task.
        let window1 = ("agent:claude", "w1");
        let window2 = ("agent:claude", "w2");
        let p = fold(&[
            create("e1", "T1", 1),
            lease("e2", "T1", 10, window1, 1_000),
            lease("e3", "T1", 20, window2, 1_000),
        ]);
        assert!(task(&p, "T1")
            .lease
            .as_ref()
            .expect("leased")
            .is_held_by("agent:claude", "w1"));
        assert!(p.noops.contains("e3"));
    }

    #[test]
    fn only_the_holder_releases() {
        let events = vec![
            create("e1", "T1", 1),
            lease("e2", "T1", 10, A, 1_000),
            ev("e3", "T1", 20, B, EventKind::Release),
        ];
        let p = fold(&events);
        assert!(task(&p, "T1").lease.is_some(), "B can't release A's lease");
        assert!(p.noops.contains("e3"));

        let mut events = events;
        events.push(ev("e4", "T1", 30, A, EventKind::Release));
        assert!(task(&fold(&events), "T1").lease.is_none());
    }

    #[test]
    fn closing_a_task_clears_its_lease() {
        for close in [
            ev("e3", "T1", 20, HUMAN, EventKind::Complete),
            state("e3", "T1", 20, State::Done),
            state("e3", "T1", 20, State::Cancelled),
        ] {
            let p = fold(&[
                create("e1", "T1", 1),
                lease("e2", "T1", 10, A, 1_000),
                close.clone(),
            ]);
            let t = task(&p, "T1");
            assert!(t.state.is_closed());
            assert!(t.lease.is_none(), "{close:?} left a lease on a closed task");
        }
    }

    #[test]
    fn a_closed_task_rejects_leases() {
        let p = fold(&[
            create("e1", "T1", 1),
            ev("e2", "T1", 2, HUMAN, EventKind::Complete),
            lease("e3", "T1", 3, A, 1_000),
        ]);
        assert!(task(&p, "T1").lease.is_none());
        assert!(p.noops.contains("e3"));
    }

    #[test]
    fn complete_is_idempotent() {
        let p = fold(&[
            create("e1", "T1", 1),
            ev("e2", "T1", 2, HUMAN, EventKind::Complete),
            ev("e3", "T1", 3, HUMAN, EventKind::Complete),
        ]);
        assert_eq!(task(&p, "T1").seq, 2);
        assert!(p.noops.contains("e3"));
    }

    // ---- claims: holds without a timer (ADR-003) ----

    #[test]
    fn a_claim_never_expires_and_keeps_others_out() {
        let far_future = 10_i64.pow(12);
        let p = fold(&[
            create("e1", "T1", 1),
            claim("e2", "T1", 10, A),
            lease("e3", "T1", far_future, B, far_future + 1_000),
            claim("e4", "T1", far_future + 1, B),
        ]);
        let l = task(&p, "T1").lease.as_ref().expect("claimed");
        assert!(l.is_held_by("agent:a", "na"));
        assert_eq!(l.expires_ms, None);
        assert!(l.is_active(i64::MAX));
        assert!(
            p.noops.contains("e3") && p.noops.contains("e4"),
            "b is refused, however late"
        );
    }

    #[test]
    fn claiming_again_changes_nothing() {
        let p = fold(&[
            create("e1", "T1", 1),
            claim("e2", "T1", 10, A),
            claim("e3", "T1", 20, A),
        ]);
        assert!(p.noops.contains("e3"));
        assert_eq!(task(&p, "T1").lease.as_ref().expect("claimed").since_ms, 10);
        assert_eq!(task(&p, "T1").seq, 2, "create + the first claim");
    }

    #[test]
    fn a_claim_takes_over_an_expired_legacy_lease() {
        // 0.1.x ledgers hold timed leases; they still expire as they always did.
        let p = fold(&[
            create("e1", "T1", 1),
            lease("e2", "T1", 10, A, 100),
            claim("e3", "T1", 200, B),
        ]);
        let l = task(&p, "T1").lease.as_ref().expect("claimed");
        assert!(l.is_held_by("agent:b", "nb"));
        assert_eq!((l.expires_ms, l.since_ms), (None, 200));
    }

    #[test]
    fn a_legacy_holder_who_claims_keeps_an_unbroken_hold() {
        let p = fold(&[
            create("e1", "T1", 1),
            lease("e2", "T1", 10, A, 1_000),
            claim("e3", "T1", 50, A),
        ]);
        let l = task(&p, "T1").lease.as_ref().expect("claimed");
        assert_eq!(l.expires_ms, None, "the lease became a claim");
        assert_eq!(l.since_ms, 10, "held without a break since the lease");
    }

    #[test]
    fn a_reclaim_takes_back_only_the_claim_it_names() {
        let p = fold(&[
            create("e1", "T1", 1),
            claim("e2", "T1", 10, A),
            reclaim("e3", "T1", 20, HUMAN, B), // b holds nothing here
            reclaim("e4", "T1", 30, HUMAN, A),
            reclaim("e5", "T1", 40, HUMAN, A), // already taken back
        ]);
        assert!(task(&p, "T1").lease.is_none());
        let expected: BTreeSet<String> = ["e3".to_string(), "e5".to_string()].into();
        assert_eq!(p.noops, expected);
    }

    #[test]
    fn a_reclaim_cannot_clobber_a_newer_claim() {
        // A reclaim racing a release: by the time it lands, b holds the task.
        let p = fold(&[
            create("e1", "T1", 1),
            claim("e2", "T1", 10, A),
            ev("e3", "T1", 20, A, EventKind::Release),
            claim("e4", "T1", 30, B),
            reclaim("e5", "T1", 40, HUMAN, A),
        ]);
        let l = task(&p, "T1").lease.as_ref().expect("claimed");
        assert!(l.is_held_by("agent:b", "nb"));
        assert!(p.noops.contains("e5"));
    }

    #[test]
    fn closing_a_task_clears_its_claim() {
        let p = fold(&[
            create("e1", "T1", 1),
            claim("e2", "T1", 10, A),
            state("e3", "T1", 20, State::Cancelled),
            claim("e4", "T1", 30, B),
        ]);
        assert!(task(&p, "T1").lease.is_none());
        assert!(p.noops.contains("e4"), "a closed task can't be claimed");
    }

    #[test]
    fn the_holders_events_on_its_task_are_its_signs_of_life() {
        let note =
            |eid: &str, ts, who| ev(eid, "T1", ts, who, EventKind::Note { text: "n".into() });
        let p = fold(&[
            create("e1", "T1", 1),
            claim("e2", "T1", 10, A),
            note("e3", 50, A),
            note("e4", 70, B),        // someone else's note says nothing about a
            claim("e5", "T1", 80, B), // nor does b's refused claim
            claim("e6", "T1", 90, A), // a no-op, but a is clearly still there
        ]);
        let l = task(&p, "T1").lease.as_ref().expect("claimed");
        assert_eq!((l.since_ms, l.last_seen_ms), (10, 90));
        assert!(p.noops.contains("e6"), "being seen is not a change");
    }

    // ---- derived blocked ----

    #[test]
    fn blocked_is_derived_and_clears_when_the_blocker_closes() {
        let base = vec![
            create("e1", "T1", 1),
            create("e2", "T2", 2),
            block("e3", "T2", 3, "T1"),
        ];
        assert!(task(&fold(&base), "T2").blocked);

        for close in [State::Done, State::Cancelled] {
            let mut events = base.clone();
            events.push(state("e4", "T1", 4, close));
            assert!(
                !task(&fold(&events), "T2").blocked,
                "a {close} blocker still blocks"
            );
        }

        let mut unblocked = base.clone();
        let kind = EventKind::Unrelate {
            rel: RelType::BlockedBy,
            task: "T1".into(),
        };
        unblocked.push(ev("e4", "T2", 4, HUMAN, kind));
        assert!(!task(&fold(&unblocked), "T2").blocked);
    }

    #[test]
    fn a_closed_task_is_never_blocked() {
        // Nothing is left to wait for, even while its blocker is still open.
        let p = fold(&[
            create("e1", "T1", 1),
            create("e2", "T2", 2),
            block("e3", "T2", 3, "T1"),
            state("e4", "T2", 4, State::Cancelled),
        ]);
        assert!(!task(&p, "T2").blocked);
    }

    #[test]
    fn a_duplicate_link_never_blocks() {
        // ADR-004: only `blocked-by` drives `blocked`; `duplicate-of` is a link.
        let kind = EventKind::Relate {
            rel: RelType::DuplicateOf,
            task: "T1".into(),
        };
        let p = fold(&[
            create("e1", "T1", 1),
            create("e2", "T2", 2),
            ev("e3", "T2", 3, HUMAN, kind),
        ]);
        let t2 = task(&p, "T2");
        assert_eq!(t2.relations.len(), 1);
        assert!(!t2.blocked, "T1 is open, but T2 only duplicates it");
    }

    #[test]
    fn missing_or_self_blockers_do_not_block() {
        let p = fold(&[
            create("e1", "T1", 1),
            block("e2", "T1", 2, "NOPE"),
            block("e3", "T1", 3, "T1"),
        ]);
        assert!(!task(&p, "T1").blocked);
    }

    // ---- property: order independence + invariants over random ledgers ----

    /// A tiny deterministic PRNG (xorshift64*) — enough for property tests
    /// without pulling in a dependency.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
        fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
            &items[self.below(items.len())]
        }
    }

    fn random_ledger(rng: &mut Rng, len: usize) -> Vec<Event> {
        let tasks = ["T1", "T2", "T3", "T4"];
        let workers = [
            ("human:h", "n1"),
            ("agent:a", "n2"),
            ("agent:a", "n3"),
            ("agent:b", "n3"),
        ];
        let states = [State::Todo, State::Doing, State::Done, State::Cancelled];
        let prios = [Priority::None, Priority::Low, Priority::High];
        let labels = ["x", "y"];
        (0..len)
            .map(|i| {
                let ts = rng.below(40) as i64; // lots of same-ms collisions on purpose
                let who = *rng.pick(&workers);
                let kind = match rng.below(17) {
                    0 | 1 => EventKind::Create {
                        title: format!("t{i}"),
                        priority: *rng.pick(&prios),
                        body: None,
                        assignee: None,
                    },
                    2 => EventKind::SetTitle {
                        title: format!("title{}", rng.below(3)),
                    },
                    3 => EventKind::SetState {
                        state: *rng.pick(&states),
                    },
                    4 => EventKind::SetPriority {
                        priority: *rng.pick(&prios),
                    },
                    5 => EventKind::SetAssignee {
                        assignee: (rng.below(2) == 0).then(|| "agent:a".to_string()),
                    },
                    6 => EventKind::SetBody {
                        body: format!("b{}", rng.below(2)),
                    },
                    7 => EventKind::LabelAdd {
                        label: rng.pick(&labels).to_string(),
                    },
                    8 => EventKind::LabelRemove {
                        label: rng.pick(&labels).to_string(),
                    },
                    9 => EventKind::Relate {
                        rel: *rng.pick(&[RelType::BlockedBy, RelType::DuplicateOf]),
                        task: rng.pick(&tasks).to_string(),
                    },
                    10 => EventKind::Unrelate {
                        rel: RelType::BlockedBy,
                        task: rng.pick(&tasks).to_string(),
                    },
                    11 => EventKind::Lease {
                        holder: who.0.to_string(),
                        expires_ms: ts + rng.below(20) as i64,
                    },
                    12 => EventKind::Release,
                    13 => EventKind::Complete,
                    14 => EventKind::Claim {
                        holder: who.0.to_string(),
                    },
                    15 => {
                        let from = *rng.pick(&workers);
                        EventKind::Reclaim {
                            holder: from.0.to_string(),
                            node: from.1.to_string(),
                            reason: None,
                        }
                    }
                    _ => EventKind::Note { text: "n".into() },
                };
                let task = *rng.pick(&tasks); // explicit: Rust 1.89 infers `&[str]` otherwise
                ev(&format!("E{i:04}"), task, ts, who, kind)
            })
            .collect()
    }

    fn check_invariants(p: &Projection, seed: u64) {
        let open: BTreeSet<&str> = p
            .tasks
            .values()
            .filter(|t| !t.state.is_closed())
            .map(|t| t.id.as_str())
            .collect();
        let mut nums: Vec<u64> = p.tasks.values().map(|t| t.num).collect();
        nums.sort_unstable();
        let dense: Vec<u64> = (1..=p.tasks.len() as u64).collect();
        assert_eq!(nums, dense, "seed {seed}: handles must be exactly #1..#n");
        for t in p.tasks.values() {
            if t.state.is_closed() {
                assert!(
                    t.lease.is_none(),
                    "seed {seed}: closed task {} holds a lease",
                    t.id
                );
            }
            assert!(t.seq >= 1, "seed {seed}");
            assert!(t.updated_ms >= t.created_ms, "seed {seed}");
            if let Some(l) = &t.lease {
                assert!(
                    l.since_ms <= l.last_seen_ms,
                    "seed {seed}: {}'s holder was last seen before its hold began",
                    t.id
                );
            }
            let should_block = !t.state.is_closed()
                && t.relations.iter().any(|r| {
                    r.rel == RelType::BlockedBy && r.task != t.id && open.contains(r.task.as_str())
                });
            assert_eq!(
                t.blocked, should_block,
                "seed {seed}: blocked flag wrong for {}",
                t.id
            );
        }
    }

    #[test]
    fn any_order_gives_the_same_state_and_invariants_hold() {
        for seed in 1..=300u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let events = random_ledger(&mut rng, 60);
            let expected = fold(&events);
            check_invariants(&expected, seed);
            for _ in 0..3 {
                let mut shuffled = events.clone();
                for i in (1..shuffled.len()).rev() {
                    let j = rng.below(i + 1);
                    shuffled.swap(i, j);
                }
                assert_eq!(
                    fold(&shuffled),
                    expected,
                    "seed {seed}: fold depends on input order"
                );
            }
        }
    }
}
