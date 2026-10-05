//! How tasks are shown: JSON (the stable contract agents parse) and text (for humans).
//!
//! JSON rules (also in AGENTS.md):
//! - `--json` prints exactly one JSON document on stdout: a task object for
//!   single-task commands, an array of them for `list`; `show` adds `events`
//!   and `release` adds `released`.
//! - Within 0.6.x, fields are only ever *added* — never renamed or removed.
//!   A breaking change bumps the version and is called out in CHANGELOG.md.
//! - Derived, read-time facts are included so agents don't recompute them:
//!   `blocked`, and `lease.active` (evaluated at the moment of the command).

use crate::config::{Field, EXAMPLE, EXPORT_COLUMNS_AFTER, EXPORT_COLUMNS_BEFORE};
use crate::error::Error;
use crate::media::{self, MediaFile};
use crate::model::{Event, EventKind, Lease, Priority, RelType, Relation, State, Task};
use crate::ops::{Detail, Entry, Exported, FieldsReport, Released, Stray, ValueCount};
use chrono::{TimeZone, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

// ============================================================ JSON views

/// A task as JSON. (A view borrows from the `Task` — `'a` is that borrow —
/// so rendering copies nothing.)
#[derive(Debug, Serialize)]
pub struct TaskView<'a> {
    pub id: &'a str,
    pub num: u64,
    pub title: &'a str,
    pub body: Option<&'a str>,
    /// Store image links in the description, in reading order (ADR-008).
    /// Derived at read time — not stored in the ledger. `[]` when there are none.
    pub media: Vec<MediaView>,
    pub state: State,
    pub priority: Priority,
    pub assignee: Option<&'a str>,
    pub labels: &'a BTreeSet<String>,
    /// Declared attributes, `{"project": "dashboard"}` — `{}` when none (ADR-006).
    pub fields: &'a BTreeMap<String, String>,
    pub relations: &'a [Relation],
    pub blocked: bool,
    pub lease: Option<LeaseView<'a>>,
    pub created_ms: i64,
    pub updated_ms: i64,
    pub seq: u64,
    /// Where it was uploaded and when, `{"notion": <unix ms>}` — `{}` when never (ADR-007).
    pub exported: &'a BTreeMap<String, i64>,
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

/// One file the description links, as JSON.
#[derive(Debug, Serialize)]
pub struct MediaView {
    pub caption: String,
    pub sha256: String,
    pub mime: &'static str,
    /// `null` when the store doesn't have the file.
    pub bytes: Option<u64>,
    /// Where the file is on this machine.
    pub path: String,
}

impl MediaView {
    fn new(file: &MediaFile) -> Self {
        MediaView {
            caption: file.caption.clone(),
            sha256: file.sha256.clone(),
            mime: file.mime,
            bytes: file.bytes,
            path: file.path.display().to_string(),
        }
    }
}

impl<'a> TaskView<'a> {
    pub fn new(t: &'a Task, now_ms: i64, folder: &Path) -> Self {
        let media = t
            .body
            .as_deref()
            .map(|body| media::files_in(folder, body))
            .unwrap_or_default()
            .iter()
            .map(MediaView::new)
            .collect();
        TaskView {
            id: &t.id,
            num: t.num,
            title: &t.title,
            body: t.body.as_deref(),
            media,
            state: t.state,
            priority: t.priority,
            assignee: t.assignee.as_deref(),
            labels: &t.labels,
            fields: &t.fields,
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
            exported: &t.exported,
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
    pub fn new(d: &'a Detail, now_ms: i64, folder: &Path) -> Self {
        DetailView {
            task: TaskView::new(&d.task, now_ms, folder),
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
    pub fn new(r: &'a Released, now_ms: i64, folder: &Path) -> Self {
        ReleaseView {
            task: TaskView::new(&r.task, now_ms, folder),
            released: r.released,
        }
    }
}

/// `fields --json` (ADR-006): where the config is, each declared field with its
/// allowed values and how much each is used, and the strays.
#[derive(Debug, Serialize)]
pub struct FieldsView<'a> {
    /// The config file's path (whether or not it exists yet).
    pub config: String,
    pub fields: Vec<FieldView<'a>>,
    pub strays: &'a [Stray],
}

/// One declared field. `values` is `null` when any text is allowed.
#[derive(Debug, Serialize)]
pub struct FieldView<'a> {
    pub field: &'a str,
    pub display_name: &'a str,
    pub values: Option<&'a [String]>,
    pub counts: &'a [ValueCount],
}

impl<'a> FieldsView<'a> {
    pub fn new(r: &'a FieldsReport) -> Self {
        FieldsView {
            config: r.config.path.display().to_string(),
            fields: r
                .config
                .fields
                .iter()
                .zip(&r.counts)
                .map(|(f, counts)| FieldView {
                    field: &f.name,
                    display_name: &f.display_name,
                    values: f.values.as_deref(),
                    counts,
                })
                .collect(),
            strays: &r.strays,
        }
    }
}

/// `export --json` (ADR-007, ADR-008): the file written (`null` when there was
/// nothing new), the task objects exported, those changed since an earlier
/// export, and `media_files` — the files this command copied beside the CSV.
/// A file already there with the same bytes is left alone and omitted.
#[derive(Debug, Serialize)]
pub struct ExportView<'a> {
    pub file: Option<String>,
    pub exported: Vec<TaskView<'a>>,
    pub changed: Vec<TaskView<'a>>,
    pub media_files: Vec<String>,
}

impl<'a> ExportView<'a> {
    pub fn new(e: &'a Exported, now_ms: i64, folder: &Path) -> Self {
        ExportView {
            file: e.file.as_ref().map(|f| f.display().to_string()),
            exported: e
                .tasks
                .iter()
                .map(|t| TaskView::new(t, now_ms, folder))
                .collect(),
            changed: e
                .changed
                .iter()
                .map(|t| TaskView::new(t, now_ms, folder))
                .collect(),
            media_files: e
                .media_files
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
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
    if !t.fields.is_empty() {
        line.push_str("  ");
        line.push_str(&field_pairs(&t.fields, " "));
    }
    if !t.labels.is_empty() {
        line.push_str("  ");
        line.push_str(&join(&t.labels, ","));
    }
    line
}

/// `project=dashboard team=design` — fields the way `--field` spells them.
fn field_pairs(fields: &BTreeMap<String, String>, sep: &str) -> String {
    fields
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(sep)
}

/// The `show` block: fields, then the annotated history.
pub fn detail_text(d: &Detail, now_ms: i64, folder: &Path) -> String {
    let t = &d.task;
    let mut rows: Vec<(&str, String)> = vec![
        ("task", format!("{}  ({})", t.handle(), t.id)),
        ("title", t.title.clone()),
    ];
    if let Some(body) = &t.body {
        rows.push(("desc", body.clone()));
        for file in media::files_in(folder, body) {
            let caption = if file.caption.is_empty() {
                String::new()
            } else {
                format!(" — {}", file.caption)
            };
            rows.push(("file", format!("{}{caption}", file.path.display())));
        }
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
    if !t.fields.is_empty() {
        rows.push(("fields", field_pairs(&t.fields, ", ")));
    }
    for (to, at) in &t.exported {
        rows.push(("exported", format!("to {to}, {}", time(*at))));
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
        EventKind::SetField { field, value } => match value {
            Some(value) => format!("{field} → {value}"),
            None => format!("{field} → (cleared)"),
        },
        EventKind::Export { to } => format!("to {to}"),
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

// ============================================================ fields

/// `hippo-task fields`: each declared field and its values, how much each is
/// used, and the strays — or, with no config yet, an example to start from.
pub fn fields_text(r: &FieldsReport) -> String {
    let path = r.config.path.display();
    if r.config.fields.is_empty() {
        let mut out = format!("No fields declared yet. Declare them in {path} — for example:\n\n");
        out.push_str(EXAMPLE);
        out.push_str("\nThen set them with `hippo-task add \"…\" --field project=dashboard`.\n");
        out.push_str(&strays_text(&r.strays));
        return out;
    }
    let mut out = format!("Fields declared in {path}:\n\n");
    for (f, counts) in r.config.fields.iter().zip(&r.counts) {
        out.push_str(&f.name);
        if f.has_custom_display_name() {
            out.push_str(&format!(" (\"{}\")", f.display_name));
        }
        let uses: Vec<String> = counts.iter().map(use_text).collect();
        match (&f.values, uses.is_empty()) {
            (Some(_), _) => out.push_str(&format!(" — one of: {}", uses.join(", "))),
            (None, true) => out.push_str(" — any text (none set yet)"),
            (None, false) => out.push_str(&format!(" — any text: {}", uses.join(", "))),
        }
        out.push('\n');
    }
    out.push_str(
        "\nSet one with `--field name=value`; find tasks with `list --field name=value`.\n",
    );
    out.push_str(&strays_text(&r.strays));
    out
}

/// `dashboard (1 open of 2)`, or `billing (unused)`.
fn use_text(c: &ValueCount) -> String {
    if c.total == 0 {
        format!("{} (unused)", c.value)
    } else {
        format!("{} ({} open of {})", c.value, c.open, c.total)
    }
}

fn strays_text(strays: &[Stray]) -> String {
    if strays.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n⚠ Values the config doesn't allow (any more):\n");
    for s in strays {
        out.push_str(&format!(
            "  #{} {}={} — set another with `hippo-task update {} --field {}=<value>`, or `--clear-field {}`\n",
            s.num, s.field, s.value, s.num, s.field, s.field
        ));
    }
    out
}

// ============================================================ export

/// The CSV Notion imports (ADR-007): a header, then one row per task. Columns:
/// Name, Status, Priority, one per declared field, Tags, Assignee, Description,
/// Blocked by, hippo-task ID.
pub fn notion_csv(tasks: &[Task], all: &BTreeMap<String, Task>, fields: &[Field]) -> String {
    let header: Vec<String> = EXPORT_COLUMNS_BEFORE
        .iter()
        .map(|c| c.to_string())
        .chain(fields.iter().map(|f| f.display_name.clone()))
        .chain(EXPORT_COLUMNS_AFTER.iter().map(|c| c.to_string()))
        .collect();
    let mut out = csv_row(&header);
    for t in tasks {
        out.push_str(&csv_row(&notion_row(t, all, fields)));
    }
    out
}

/// One task's cells, in [`notion_csv`]'s column order. `all` is every task, as
/// of the same moment, for the `Blocked by` cell.
pub fn notion_row(t: &Task, all: &BTreeMap<String, Task>, fields: &[Field]) -> Vec<String> {
    let mut row = vec![
        t.title.clone(),
        notion_status(t.state).to_string(),
        notion_priority(t.priority).to_string(),
    ];
    row.extend(
        fields
            .iter()
            .map(|f| t.fields.get(&f.name).cloned().unwrap_or_default()),
    );
    row.push(join(&t.labels, ", ")); // Notion splits a multi-select on commas
    row.push(t.assignee.clone().unwrap_or_default());
    row.push(t.body.clone().unwrap_or_default());
    row.push(open_blockers(t, all));
    row.push(t.id.clone());
    row
}

/// Notion's default status options.
fn notion_status(s: State) -> &'static str {
    match s {
        State::Todo => "Not started",
        State::Doing => "In progress",
        State::Done => "Done",
        State::Cancelled => "Cancelled",
    }
}

fn notion_priority(p: Priority) -> &'static str {
    match p {
        Priority::None => "",
        Priority::Low => "Low",
        Priority::Med => "Medium",
        Priority::High => "High",
        Priority::Urgent => "Urgent",
    }
}

/// `#3 Write docs; #5 Ship auth` — what still blocks the task (a closed task
/// never blocks anything).
fn open_blockers(t: &Task, all: &BTreeMap<String, Task>) -> String {
    t.relations
        .iter()
        .filter(|r| r.rel == RelType::BlockedBy)
        .filter_map(|r| all.get(&r.task))
        .filter(|blocker| !blocker.state.is_closed())
        .map(|blocker| format!("{} {}", blocker.handle(), blocker.title))
        .collect::<Vec<_>>()
        .join("; ")
}

/// One CSV line (RFC 4180): cells joined by commas, each quoted only if it
/// holds a comma, a quote or a line break — with quotes inside doubled.
fn csv_row(cells: &[String]) -> String {
    let mut line = cells
        .iter()
        .map(|cell| {
            if cell.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", cell.replace('"', "\"\""))
            } else {
                cell.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    line.push('\n');
    line
}

/// What `export --out` did, for people: the file, how to import it, and the
/// tasks that changed since an earlier export.
pub fn export_text(e: &Exported) -> String {
    let mut out = match &e.file {
        Some(file) => format!(
            "exported {} to {}\n  In Notion: Import → CSV makes a new database; for an existing one, use its ••• menu → Merge with CSV (the headers must match its property names).\n",
            count(e.tasks.len(), "task"),
            file.display()
        ),
        None => "nothing new to export — every matching task is already there (--again exports them again)\n".to_string(),
    };
    if e.file.is_some()
        && e.tasks
            .iter()
            .any(|t| t.body.as_deref().is_some_and(media::has_store_media))
    {
        out.push_str("  Notion's importer doesn't upload files; they stay on disk, in media/ beside the CSV, to be added by hand.\n");
    }
    if !e.changed.is_empty() {
        out.push_str(&format!("{}\n", changed_text(&e.changed)));
    }
    out
}

/// The tasks that changed after their last export, as one sentence.
pub fn changed_text(changed: &[Task]) -> String {
    let handles: Vec<String> = changed
        .iter()
        .map(|t| format!("{} \"{}\"", t.handle(), t.title))
        .collect();
    let (they, them, rows) = if changed.len() == 1 {
        ("it was", "it", "a new row")
    } else {
        ("they were", "them", "new rows")
    };
    let again = if changed.iter().any(|t| t.state.is_closed()) {
        "--again (plus --all for closed tasks)"
    } else {
        "--again"
    };
    format!(
        "{} changed since {they} exported: {} — update {them} in Notion by hand, or add {them} again as {rows} with {again}",
        count(changed.len(), "task"),
        handles.join(", ")
    )
}

/// `1 task`, `2 tasks`.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
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
