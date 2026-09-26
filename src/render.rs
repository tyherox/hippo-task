//! How tasks are shown: JSON (the stable contract agents parse) and text (for humans).
//!
//! JSON rules (also in AGENTS.md):
//! - `--json` prints exactly one JSON document on stdout: a task object for
//!   single-task commands, an array of them for `list`; `show` adds `events`
//!   and `release` adds `released`.
//! - Within 0.1.x, fields are only ever *added* — never renamed or removed.
//!   A breaking change bumps the version and is called out in CHANGELOG.md.
//! - Derived, read-time facts are included so agents don't recompute them:
//!   `blocked`, and `lease.active` (evaluated at the moment of the command).

use crate::error::Error;
use crate::model::{Event, EventKind, Lease, Priority, Relation, State, Task};
use crate::ops::{Detail, Entry, Released};
use chrono::{TimeZone, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

// ============================================================ JSON views

/// A task as JSON. (A view borrows from the `Task` — `'a` is that borrow —
/// so rendering copies nothing.)
#[derive(Debug, Serialize)]
pub struct TaskView<'a> {
    pub id: &'a str,
    pub num: u64,
    pub title: &'a str,
    pub body: Option<&'a str>,
    pub state: State,
    pub priority: Priority,
    pub assignee: Option<&'a str>,
    pub labels: &'a BTreeSet<String>,
    pub relations: &'a [Relation],
    pub blocked: bool,
    pub lease: Option<LeaseView<'a>>,
    pub created_ms: i64,
    pub updated_ms: i64,
    pub seq: u64,
}

/// A hold as JSON, with `active` evaluated at render time. `expires_ms` is
/// `null` for a claim (0.2.0 — no timer) and a unix-millis instant for a
/// 0.1.x lease. (The field keeps the name `lease` so agents' parsers don't break.)
#[derive(Debug, Serialize)]
pub struct LeaseView<'a> {
    pub holder: &'a str,
    pub node: &'a str,
    pub expires_ms: Option<i64>,
    pub active: bool,
    pub since_ms: i64,
    pub last_seen_ms: i64,
}

impl<'a> TaskView<'a> {
    pub fn new(t: &'a Task, now_ms: i64) -> Self {
        TaskView {
            id: &t.id,
            num: t.num,
            title: &t.title,
            body: t.body.as_deref(),
            state: t.state,
            priority: t.priority,
            assignee: t.assignee.as_deref(),
            labels: &t.labels,
            relations: &t.relations,
            blocked: t.blocked,
            lease: t.lease.as_ref().map(|l| LeaseView {
                holder: &l.holder,
                node: &l.node,
                expires_ms: l.expires_ms,
                active: l.is_active(now_ms),
                since_ms: l.since_ms,
                last_seen_ms: l.last_seen_ms,
            }),
            created_ms: t.created_ms,
            updated_ms: t.updated_ms,
            seq: t.seq,
        }
    }
}

/// `show --json`: the task's fields, plus its annotated event history.
#[derive(Debug, Serialize)]
pub struct DetailView<'a> {
    #[serde(flatten)]
    pub task: TaskView<'a>,
    pub events: Vec<EventView<'a>>,
}

/// One history event: exactly the on-disk event, plus `applied`.
#[derive(Debug, Serialize)]
pub struct EventView<'a> {
    #[serde(flatten)]
    pub event: &'a Event,
    /// false = recorded but changed nothing (a rejected lease, a repeat, …).
    pub applied: bool,
}

impl<'a> DetailView<'a> {
    pub fn new(d: &'a Detail, now_ms: i64) -> Self {
        DetailView {
            task: TaskView::new(&d.task, now_ms),
            events: d
                .history
                .iter()
                .map(|e| EventView {
                    event: &e.event,
                    applied: e.applied,
                })
                .collect(),
        }
    }
}

/// `release --json`: the task's fields, plus whether the caller held the lease.
#[derive(Debug, Serialize)]
pub struct ReleaseView<'a> {
    #[serde(flatten)]
    pub task: TaskView<'a>,
    /// false = you didn't hold it, so nothing changed.
    pub released: bool,
}

impl<'a> ReleaseView<'a> {
    pub fn new(r: &'a Released, now_ms: i64) -> Self {
        ReleaseView {
            task: TaskView::new(&r.task, now_ms),
            released: r.released,
        }
    }
}

/// An error as JSON (stderr, `--json` mode): `{"error":"conflict","message":…,"exit_code":4}`.
#[derive(Debug, Serialize)]
pub struct ErrorView {
    pub error: &'static str,
    pub message: String,
    pub exit_code: u8,
}

impl ErrorView {
    pub fn new(e: &Error) -> Self {
        ErrorView {
            error: e.kind(),
            message: e.to_string(),
            exit_code: e.exit_code(),
        }
    }
}

// ============================================================ text

/// A timestamp for humans, always in UTC and labelled as such.
pub fn time(ms: i64) -> String {
    match Utc.timestamp_millis_opt(ms).single() {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => format!("{ms} (unix ms)"),
    }
}

/// A worker, e.g. `agent:claude@cc` (actor @ node).
pub fn who(actor: &str, node: &str) -> String {
    format!("{actor}@{node}")
}

/// Time left on a lease, rounded up: `9m left`, `40s left`, `expired`.
pub fn remaining(expires_ms: i64, now_ms: i64) -> String {
    match short_remaining(expires_ms, now_ms) {
        Some(left) => format!("{left} left"),
        None => "expired".to_string(),
    }
}

fn short_remaining(expires_ms: i64, now_ms: i64) -> Option<String> {
    let left = expires_ms.saturating_sub(now_ms);
    if left <= 0 {
        None
    } else if left < 60_000 {
        Some(format!("{}s", (left + 999) / 1_000))
    } else {
        Some(format!("{}m", (left + 59_999) / 60_000))
    }
}

/// How a hold stands, for messages: a claim's quiet time (`quiet 12m`, or
/// `just seen`), or a 0.1.x lease's time left (`9m left`, `expired`).
pub fn hold_status(l: &Lease, now_ms: i64) -> String {
    match l.expires_ms {
        None => ago(l.last_seen_ms, now_ms)
            .map_or_else(|| "just seen".to_string(), |quiet| format!("quiet {quiet}")),
        Some(expires_ms) => remaining(expires_ms, now_ms),
    }
}

/// How long ago `then_ms` was, coarsely — `12m`, `3h`, `2d` — or `None` under a minute.
fn ago(then_ms: i64, now_ms: i64) -> Option<String> {
    match now_ms.saturating_sub(then_ms) / 60_000 {
        m if m < 1 => None,
        m if m < 60 => Some(format!("{m}m")),
        m if m < 48 * 60 => Some(format!("{}h", m / 60)),
        m => Some(format!("{}d", m / (24 * 60))),
    }
}

/// Compact hold badge for `list`: a claim is `held:agent:claude@cc`, plus
/// `(quiet 12m)` once its holder has been quiet a while; a 0.1.x lease is
/// `lease:agent:claude@cc(9m)` or `…(expired)`.
pub fn lease_badge(l: &Lease, now_ms: i64) -> String {
    let worker = who(&l.holder, &l.node);
    match l.expires_ms {
        None => match ago(l.last_seen_ms, now_ms) {
            Some(quiet) => format!("held:{worker}(quiet {quiet})"),
            None => format!("held:{worker}"),
        },
        Some(expires_ms) => {
            let left = short_remaining(expires_ms, now_ms).unwrap_or_else(|| "expired".to_string());
            format!("lease:{worker}({left})")
        }
    }
}

/// One `list` line: `#3    doing     high   Ship auth [blocked] lease:…  backend,api`.
pub fn list_line(t: &Task, now_ms: i64) -> String {
    let mut line = format!(
        "{:<5} {:<9} {:<6} {}",
        t.handle(),
        t.state,
        t.priority,
        t.title
    );
    if t.blocked {
        line.push_str(" [blocked]");
    }
    if let Some(l) = &t.lease {
        line.push(' ');
        line.push_str(&lease_badge(l, now_ms));
    }
    if !t.labels.is_empty() {
        line.push_str("  ");
        line.push_str(&join(&t.labels, ","));
    }
    line
}

/// The `show` block: fields, then the annotated history.
pub fn detail_text(d: &Detail, now_ms: i64) -> String {
    let t = &d.task;
    let mut rows: Vec<(&str, String)> = vec![
        ("task", format!("{}  ({})", t.handle(), t.id)),
        ("title", t.title.clone()),
    ];
    if let Some(body) = &t.body {
        rows.push(("desc", body.clone()));
    }
    let blocked = if t.blocked { " (blocked)" } else { "" };
    rows.push(("state", format!("{}{blocked}", t.state)));
    rows.push(("priority", t.priority.to_string()));
    if let Some(a) = &t.assignee {
        rows.push(("assignee", a.clone()));
    }
    if let Some(l) = &t.lease {
        let worker = who(&l.holder, &l.node);
        let hold = match l.expires_ms {
            None => format!(
                "{worker} — claimed {}, {}",
                time(l.since_ms),
                hold_status(l, now_ms)
            ),
            Some(expires_ms) => format!(
                "{worker} — {} (until {})",
                remaining(expires_ms, now_ms),
                time(expires_ms)
            ),
        };
        rows.push(("held", hold));
    }
    if !t.labels.is_empty() {
        rows.push(("labels", join(&t.labels, ", ")));
    }
    for r in &t.relations {
        rows.push((
            "rel",
            format!("{} {}", r.rel, describe_ref(&d.all, &r.task)),
        ));
    }
    rows.push(("created", time(t.created_ms)));
    rows.push((
        "updated",
        format!("{}  (seq {})", time(t.updated_ms), t.seq),
    ));

    let mut out = String::new();
    for (key, value) in rows {
        out.push_str(&format!("{:<9} {value}\n", format!("{key}:")));
    }
    out.push_str("--- history ---\n");
    for entry in &d.history {
        out.push_str(&history_line(entry, &d.all));
        out.push('\n');
    }
    out
}

/// One audit line: when, what, who, detail — and whether it had any effect.
fn history_line(entry: &Entry, all: &BTreeMap<String, Task>) -> String {
    let ev = &entry.event;
    let detail = match &ev.kind {
        EventKind::Create { title, .. } => format!("\"{title}\""),
        EventKind::SetTitle { title } => format!("→ \"{title}\""),
        EventKind::SetState { state } => format!("→ {state}"),
        EventKind::SetPriority { priority } => format!("→ {priority}"),
        EventKind::SetAssignee { assignee } => {
            format!("→ {}", assignee.as_deref().unwrap_or("(nobody)"))
        }
        EventKind::SetBody { body } => format!("→ \"{}\"", clip(body, 60)),
        EventKind::LabelAdd { label } => format!("+{label}"),
        EventKind::LabelRemove { label } => format!("-{label}"),
        EventKind::Relate { rel, task } => format!("{rel} {}", handle_of(all, task)),
        EventKind::Unrelate { rel, task } => format!("no longer {rel} {}", handle_of(all, task)),
        EventKind::Lease { expires_ms, .. } => {
            format!(
                "for {}m",
                (expires_ms.saturating_sub(ev.ts) + 59_999) / 60_000
            )
        }
        EventKind::Claim { .. } => String::new(),
        EventKind::Reclaim {
            holder,
            node,
            reason,
        } => match reason {
            Some(why) => format!("from {} — \"{why}\"", who(holder, node)),
            None => format!("from {}", who(holder, node)),
        },
        EventKind::Note { text } => format!("\"{text}\""),
        EventKind::Release | EventKind::Complete => String::new(),
    };
    let effect = match (&ev.kind, entry.applied) {
        (_, true) => "",
        (EventKind::Lease { .. } | EventKind::Claim { .. }, false) => "  (rejected)",
        (EventKind::Reclaim { .. }, false) => "  (they no longer held it — no effect)",
        (EventKind::Release, false) => "  (not the holder — no effect)",
        (_, false) => "  (no change)",
    };
    let line = format!(
        "{}  {:<12} {:<24} {detail}{effect}",
        time(ev.ts),
        ev.kind.name(),
        who(&ev.actor, &ev.node)
    );
    line.trim_end().to_string()
}

fn describe_ref(all: &BTreeMap<String, Task>, id: &str) -> String {
    match all.get(id) {
        Some(o) => format!("{} \"{}\" ({})", o.handle(), o.title, o.state),
        None => format!("{id} (unknown task)"),
    }
}

fn handle_of(all: &BTreeMap<String, Task>, id: &str) -> String {
    all.get(id).map_or_else(|| id.to_string(), Task::handle)
}

fn join(items: &BTreeSet<String>, sep: &str) -> String {
    items
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(sep)
}

/// First `max` characters on one line (char-safe — never splits a UTF-8 char).
fn clip(s: &str, max: usize) -> String {
    let flat = s.replace('\n', " ");
    if flat.chars().count() <= max {
        flat
    } else {
        let mut cut: String = flat.chars().take(max).collect();
        cut.push('…');
        cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_rounds_up_and_says_expired() {
        assert_eq!(remaining(10 * 60_000, 0), "10m left");
        assert_eq!(remaining(9 * 60_000 + 1, 0), "10m left");
        assert_eq!(remaining(40_000, 0), "40s left");
        assert_eq!(remaining(0, 0), "expired");
        assert_eq!(remaining(-5, 0), "expired");
    }

    #[test]
    fn time_is_labelled_utc() {
        assert_eq!(time(0), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn clip_is_char_safe() {
        assert_eq!(clip("héllo wörld", 5), "héllo…");
        assert_eq!(clip("a\nb", 10), "a b");
    }
}
