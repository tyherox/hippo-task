//! Data model for the task schema.
//!
//! The big idea: the **Event is the source of truth**; the **Task is a
//! projection** you rebuild by folding events (see `fold.rs`). Nothing on disk
//! is ever edited in place — you only ever append events.
//!
//! Rust notes:
//! - `#[derive(...)]` auto-implements traits. `Serialize`/`Deserialize` are
//!   serde's; `clap::ValueEnum` lets an enum be a CLI argument.
//! - `#[serde(rename_all = "lowercase")]` controls the JSON spelling. The
//!   `as_str` methods below must spell things the same way — the tests at the
//!   bottom of this file check that, so text output and JSON can't drift apart.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Canonical task lifecycle. NOTE: "blocked" is intentionally NOT a state —
/// it is *derived* (see `fold.rs`), because "blocked" has no clean interop
/// mapping to VTODO/OSLC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Todo,
    Doing,
    Done,
    Cancelled,
}

impl State {
    /// Done and cancelled are *closed*: no more work happens on them, so they
    /// never hold a lease and never block other tasks.
    pub fn is_closed(self) -> bool {
        matches!(self, State::Done | State::Cancelled)
    }

    /// The spelling used on the command line and in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            State::Todo => "todo",
            State::Doing => "doing",
            State::Done => "done",
            State::Cancelled => "cancelled",
        }
    }
}

/// Priority. Declaration order is significance order, so deriving `Ord` gives
/// `None < Low < Med < High < Urgent` for free (`list --sort priority` uses it).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    None,
    Low,
    Med,
    High,
    Urgent,
}

impl Priority {
    /// The spelling used on the command line and in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::None => "none",
            Priority::Low => "low",
            Priority::Med => "med",
            Priority::High => "high",
            Priority::Urgent => "urgent",
        }
    }
}

/// How two tasks relate. (Only `blocked-by` has behaviour today: it drives
/// the derived `blocked` flag. The rest are reserved for interop.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelType {
    Parent,
    Child,
    Blocks,
    BlockedBy,
    RelatesTo,
}

impl RelType {
    /// The spelling used in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            RelType::Parent => "parent",
            RelType::Child => "child",
            RelType::Blocks => "blocks",
            RelType::BlockedBy => "blocked-by",
            RelType::RelatesTo => "relates-to",
        }
    }
}

// `Display` lets you write `format!("{}", state)`. `f.pad` (not `write_str`)
// respects width/alignment, so `format!("{:<9}", state)` lines up in `list`.
impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}
impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}
impl fmt::Display for RelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub rel: RelType,
    pub task: String,
}

/// Who holds a task right now — "who is actively doing this".
/// Distinct from `Task.assignee` ("who *should* own it", durable intent).
///
/// A hold belongs to a **worker = actor + node**. Two windows of the same
/// agent (`agent:claude` on nodes `w1` and `w2`) are two workers, and the
/// hold is what stops them from doing the same task twice.
///
/// Since 0.2.0 a hold is a **claim**: no timer. It lasts until the worker
/// releases it, the task closes, or someone reclaims it (ADR-003). Ledgers
/// written by 0.1.x also hold timed **leases**, which still expire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub holder: String,
    pub node: String,
    /// `None` for a claim; for a 0.1.x lease, the first instant it no longer holds.
    pub expires_ms: Option<i64>,
    /// When this worker's unbroken hold began.
    pub since_ms: i64,
    /// The holder's latest event on this task — its last sign of life.
    pub last_seen_ms: i64,
}

impl Lease {
    /// Still in force at `now_ms`? A claim always is; a lease until it expires.
    pub fn is_active(&self, now_ms: i64) -> bool {
        self.expires_ms.is_none_or(|expires| expires > now_ms)
    }

    /// Held by this worker (same actor on the same node)?
    pub fn is_held_by(&self, actor: &str, node: &str) -> bool {
        self.holder == actor && self.node == node
    }
}

/// The projected task. Derived from events; never edited directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String, // ULID — the permanent id
    pub num: u64,   // derived: the Nth task created → the friendly `#N` handle
    pub title: String,
    pub body: Option<String>, // the description
    pub state: State,
    pub priority: Priority,
    pub assignee: Option<String>, // durable intent (who *should* own it)
    pub labels: BTreeSet<String>, // a SET → concurrent adds don't clobber
    pub relations: Vec<Relation>,
    pub lease: Option<Lease>, // ephemeral: who's *actively* doing it now
    pub created_ms: i64,
    pub updated_ms: i64,
    pub seq: u64,      // count of content changes (interop SEQUENCE)
    pub blocked: bool, // derived after the fold
}

impl Task {
    /// The friendly handle, e.g. `#12`. Stable on one machine; the ULID `id`
    /// is the permanent identifier (handles may renumber once multi-machine
    /// sync exists — see schema-design.md §11).
    pub fn handle(&self) -> String {
        format!("#{}", self.num)
    }
}

/// One appended fact. Immutable. Ordered by `(ts, node, eid)`.
///
/// `#[serde(flatten)]` merges the tagged `kind` fields up to the top level, so
/// on disk an event reads like:
///   {"eid":"…","task":"…","ts":…,"actor":"…","node":"…","type":"create","data":{…}}
///
/// This shape is the interop contract — it is frozen by a test below.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub eid: String,   // ULID: globally unique, created by the writer
    pub task: String,  // the task this event applies to
    pub ts: i64,       // unix millis; strictly increasing per ledger (store.rs)
    pub actor: String, // SEMANTIC who (human:x | agent:y) — for audit/display
    pub node: String,  // PHYSICAL writer instance (window/session) — the tiebreak
    #[serde(flatten)]
    pub kind: EventKind,
}

/// The verbs. `tag = "type", content = "data"` → `{"type":"create","data":{…}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "kebab-case")]
pub enum EventKind {
    Create {
        title: String,
        priority: Priority,
        body: Option<String>,
        assignee: Option<String>,
    },
    SetTitle {
        title: String,
    },
    SetState {
        state: State,
    },
    SetPriority {
        priority: Priority,
    },
    SetAssignee {
        assignee: Option<String>,
    },
    SetBody {
        body: String,
    },
    LabelAdd {
        label: String,
    },
    LabelRemove {
        label: String,
    },
    Relate {
        rel: RelType,
        task: String,
    },
    Unrelate {
        rel: RelType,
        task: String,
    },
    /// A timed hold, as 0.1.x wrote it. Still read, no longer written.
    Lease {
        holder: String,
        expires_ms: i64,
    },
    /// A hold with no timer (ADR-003).
    Claim {
        holder: String,
    },
    /// Take back the hold of the worker named here — for whoever knows it's
    /// gone. Naming the holder means a stale reclaim can't clobber a newer claim.
    Reclaim {
        holder: String,
        node: String,
        reason: Option<String>,
    },
    Note {
        text: String,
    },
    Release,
    Complete,
}

impl EventKind {
    /// The on-disk `type` tag, e.g. `"set-title"` (used by the history view).
    pub fn name(&self) -> &'static str {
        match self {
            EventKind::Create { .. } => "create",
            EventKind::SetTitle { .. } => "set-title",
            EventKind::SetState { .. } => "set-state",
            EventKind::SetPriority { .. } => "set-priority",
            EventKind::SetAssignee { .. } => "set-assignee",
            EventKind::SetBody { .. } => "set-body",
            EventKind::LabelAdd { .. } => "label-add",
            EventKind::LabelRemove { .. } => "label-remove",
            EventKind::Relate { .. } => "relate",
            EventKind::Unrelate { .. } => "unrelate",
            EventKind::Lease { .. } => "lease",
            EventKind::Claim { .. } => "claim",
            EventKind::Reclaim { .. } => "reclaim",
            EventKind::Note { .. } => "note",
            EventKind::Release => "release",
            EventKind::Complete => "complete",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every event kind — if you add a kind, add it here too.
    fn every_kind() -> Vec<EventKind> {
        vec![
            EventKind::Create {
                title: "t".into(),
                priority: Priority::High,
                body: None,
                assignee: None,
            },
            EventKind::SetTitle { title: "t".into() },
            EventKind::SetState {
                state: State::Doing,
            },
            EventKind::SetPriority {
                priority: Priority::Low,
            },
            EventKind::SetAssignee { assignee: None },
            EventKind::SetBody { body: "b".into() },
            EventKind::LabelAdd { label: "l".into() },
            EventKind::LabelRemove { label: "l".into() },
            EventKind::Relate {
                rel: RelType::BlockedBy,
                task: "x".into(),
            },
            EventKind::Unrelate {
                rel: RelType::BlockedBy,
                task: "x".into(),
            },
            EventKind::Lease {
                holder: "agent:a".into(),
                expires_ms: 1,
            },
            EventKind::Claim {
                holder: "agent:a".into(),
            },
            EventKind::Reclaim {
                holder: "agent:a".into(),
                node: "n1".into(),
                reason: Some("window closed".into()),
            },
            EventKind::Note { text: "n".into() },
            EventKind::Release,
            EventKind::Complete,
        ]
    }

    #[test]
    fn event_names_match_the_on_disk_type_tags() {
        for kind in every_kind() {
            let json = serde_json::to_value(&kind).unwrap();
            assert_eq!(
                json["type"],
                kind.name(),
                "name() drifted from serde for {kind:?}"
            );
        }
    }

    #[test]
    fn text_spellings_match_json_spellings() {
        for s in [State::Todo, State::Doing, State::Done, State::Cancelled] {
            assert_eq!(serde_json::to_value(s).unwrap(), s.as_str());
        }
        for p in [
            Priority::None,
            Priority::Low,
            Priority::Med,
            Priority::High,
            Priority::Urgent,
        ] {
            assert_eq!(serde_json::to_value(p).unwrap(), p.as_str());
        }
        for r in [
            RelType::Parent,
            RelType::Child,
            RelType::Blocks,
            RelType::BlockedBy,
            RelType::RelatesTo,
        ] {
            assert_eq!(serde_json::to_value(r).unwrap(), r.as_str());
        }
    }

    #[test]
    fn display_pads_for_aligned_columns() {
        assert_eq!(format!("[{:<6}]", State::Todo), "[todo  ]");
        assert_eq!(format!("[{:<6}]", Priority::Urgent), "[urgent]");
    }

    #[test]
    fn priority_orders_by_significance() {
        assert!(Priority::None < Priority::Low);
        assert!(Priority::Low < Priority::Med);
        assert!(Priority::Med < Priority::High);
        assert!(Priority::High < Priority::Urgent);
    }

    /// The ledger line format is the interop contract. If this test fails you
    /// are about to break every existing ledger — don't "fix" the test.
    #[test]
    fn on_disk_event_shape_is_frozen() {
        let ev = Event {
            eid: "E1".into(),
            task: "T1".into(),
            ts: 5,
            actor: "agent:a".into(),
            node: "n1".into(),
            kind: EventKind::LabelAdd { label: "x".into() },
        };
        let line = serde_json::to_string(&ev).unwrap();
        assert_eq!(
            line,
            r#"{"eid":"E1","task":"T1","ts":5,"actor":"agent:a","node":"n1","type":"label-add","data":{"label":"x"}}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&line).unwrap(), ev);

        // Unit variants carry no `data`.
        let done = Event {
            kind: EventKind::Complete,
            ..ev.clone()
        };
        let line = serde_json::to_string(&done).unwrap();
        assert_eq!(
            line,
            r#"{"eid":"E1","task":"T1","ts":5,"actor":"agent:a","node":"n1","type":"complete"}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&line).unwrap(), done);

        // 0.2.0's kinds (ADR-003) are frozen from their first release, too.
        let claim = Event {
            kind: EventKind::Claim {
                holder: "agent:a".into(),
            },
            ..ev.clone()
        };
        let line = serde_json::to_string(&claim).unwrap();
        assert_eq!(
            line,
            r#"{"eid":"E1","task":"T1","ts":5,"actor":"agent:a","node":"n1","type":"claim","data":{"holder":"agent:a"}}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&line).unwrap(), claim);
        let reclaim = Event {
            kind: EventKind::Reclaim {
                holder: "agent:b".into(),
                node: "n2".into(),
                reason: Some("window closed".into()),
            },
            ..ev.clone()
        };
        let line = serde_json::to_string(&reclaim).unwrap();
        assert_eq!(
            line,
            r#"{"eid":"E1","task":"T1","ts":5,"actor":"agent:a","node":"n1","type":"reclaim","data":{"holder":"agent:b","node":"n2","reason":"window closed"}}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&line).unwrap(), reclaim);
    }

    #[test]
    fn every_kind_round_trips_through_json() {
        for kind in every_kind() {
            let ev = Event {
                eid: "E".into(),
                task: "T".into(),
                ts: 1,
                actor: "a".into(),
                node: "n".into(),
                kind,
            };
            let back: Event = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
            assert_eq!(back, ev);
        }
    }

    #[test]
    fn lease_belongs_to_actor_and_node() {
        let l = Lease {
            holder: "agent:claude".into(),
            node: "w1".into(),
            expires_ms: Some(100),
            since_ms: 0,
            last_seen_ms: 0,
        };
        assert!(l.is_held_by("agent:claude", "w1"));
        assert!(
            !l.is_held_by("agent:claude", "w2"),
            "another window is another worker"
        );
        assert!(!l.is_held_by("agent:codex", "w1"));
        assert!(l.is_active(99));
        assert!(
            !l.is_active(100),
            "expires_ms is the first instant it no longer holds"
        );
    }

    #[test]
    fn a_claim_has_no_expiry() {
        // ADR-003: a claim holds until it's released, the task closes, or
        // someone reclaims it — no amount of time passing ends it.
        let claim = Lease {
            holder: "agent:claude".into(),
            node: "w1".into(),
            expires_ms: None,
            since_ms: 0,
            last_seen_ms: 0,
        };
        assert!(claim.is_active(0));
        assert!(claim.is_active(i64::MAX));
    }
}
