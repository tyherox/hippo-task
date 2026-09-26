//! The commands' rules: resolve ids, validate input, enforce etiquette, append events.
//!
//! Division of labour (worth internalizing):
//! - `fold` is **physics** — deterministic merge rules that hold for *any*
//!   ledger, including ones written by other tools or merged from elsewhere.
//! - `ops` is **etiquette** — what *this* CLI lets you do: don't close a task
//!   someone else is working on, don't block a task on itself, no empty titles.
//!
//! Every write runs inside one store transaction (exclusive lock → read →
//! decide → append + fsync), so every decision is made against the latest
//! state, and a lease verdict can't be overturned by a write that lands later.
//!
//! Nothing here reads the clock: `Ctx::now_ms` is passed in, so tests control time.

use crate::error::{Error, Result};
use crate::fold::{self, Projection};
use crate::model::{Event, EventKind, Priority, RelType, State, Task};
use crate::render::{remaining, who};
use crate::store::{Store, Tx};
use std::collections::BTreeMap;
use ulid::Ulid;

/// Longest lease you can take in one go (renew to hold it longer).
pub const MAX_LEASE_MINUTES: i64 = 24 * 60;

/// Shortest id suffix accepted, so a stray `hippo-task done A` can't hit a random task.
const MIN_SUFFIX: usize = 4;

/// Who is acting, and when. Built once per command by the CLI.
#[derive(Debug, Clone)]
pub struct Ctx {
    /// Semantic who: `human:local`, `agent:claude`, …
    pub actor: String,
    /// Which writer instance (window / session). A lease belongs to actor + node.
    pub node: String,
    /// Wall-clock "now" in unix millis.
    pub now_ms: i64,
}

impl Ctx {
    /// Who is acting. Privacy by default: without `--actor`/`HIPPO_ACTOR` the
    /// ledger records `human:local` — never `$USER`, a hostname, or anything else
    /// taken from the machine. (Guarded by a test in tests/cli.rs.)
    ///
    /// Agents must name their node: a lease belongs to actor + node, so if every
    /// window of `agent:claude` defaulted to the same node they'd be one worker,
    /// and the lease would no longer stop two windows taking the same task.
    pub fn new(actor: Option<String>, node: Option<String>, now_ms: i64) -> Result<Ctx> {
        let actor = actor.unwrap_or_else(|| "human:local".to_string());
        if node.is_none() && actor.trim().starts_with("agent:") {
            return Err(Error::Usage(format!(
                "agents must name their node: set HIPPO_NODE (or --node) to something unique per window/session, e.g. HIPPO_NODE={}-1 — see AGENTS.md",
                actor.trim().trim_start_matches("agent:")
            )));
        }
        let node = node.unwrap_or_else(|| "local".to_string());
        for (flag, value) in [
            ("--actor / HIPPO_ACTOR", &actor),
            ("--node / HIPPO_NODE", &node),
        ] {
            if value.trim().is_empty() {
                return Err(Error::Usage(format!("{flag} must not be empty")));
            }
        }
        Ok(Ctx {
            actor: actor.trim().to_string(),
            node: node.trim().to_string(),
            now_ms,
        })
    }

    /// An event stamped inside the transaction (monotonic timestamp).
    fn event(&self, tx: &mut Tx, task: &str, kind: EventKind) -> Event {
        let ts = tx.next_ts(self.now_ms);
        self.event_at(ts, task, kind)
    }

    /// A lease event; expiry counts from the event's own timestamp.
    fn lease_event(&self, tx: &mut Tx, task: &str, minutes: i64) -> Event {
        let ts = tx.next_ts(self.now_ms);
        let kind = EventKind::Lease {
            holder: self.actor.clone(),
            expires_ms: ts.saturating_add(minutes.saturating_mul(60_000)),
        };
        self.event_at(ts, task, kind)
    }

    fn event_at(&self, ts: i64, task: &str, kind: EventKind) -> Event {
        Event {
            eid: Ulid::new().to_string(),
            task: task.to_string(),
            ts,
            actor: self.actor.clone(),
            node: self.node.clone(),
            kind,
        }
    }
}

// ------------------------------------------------------------------ add

/// Input for [`add`].
#[derive(Debug, Clone)]
pub struct NewTask {
    pub title: String,
    pub priority: Priority,
    pub body: Option<String>,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
}

/// Create a task (plus one event per label, in the same transaction).
pub fn add(store: &Store, ctx: &Ctx, new: NewTask) -> Result<Task> {
    let title = required("title", &new.title)?;
    let labels = new
        .labels
        .iter()
        .map(|l| required("label", l))
        .collect::<Result<Vec<_>>>()?;
    let assignee = optional("assignee", new.assignee.as_deref())?;
    let body = optional("description", new.body.as_deref())?;

    let id = Ulid::new().to_string();
    let mut tx = store.begin()?;
    let create = EventKind::Create {
        title,
        priority: new.priority,
        body,
        assignee,
    };
    let mut events = vec![ctx.event(&mut tx, &id, create)];
    for label in labels {
        events.push(ctx.event(&mut tx, &id, EventKind::LabelAdd { label }));
    }
    let ledger = tx.commit(events)?;
    task_in(fold::fold(&ledger.events), &id)
}

// --------------------------------------------------------------- update

/// Input for [`update`]. Every field is optional; at least one must be set.
#[derive(Debug, Clone, Default)]
pub struct Changes {
    pub title: Option<String>,
    pub state: Option<State>,
    pub priority: Option<Priority>,
    pub assignee: Option<String>,
    pub unassign: bool,
    pub body: Option<String>,
    pub label_add: Vec<String>,
    pub label_remove: Vec<String>,
    pub block: Vec<String>,
    pub unblock: Vec<String>,
    /// With `state`: change it even if another worker holds the lease.
    pub force: bool,
}

impl Changes {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.state.is_none()
            && self.priority.is_none()
            && self.assignee.is_none()
            && !self.unassign
            && self.body.is_none()
            && self.label_add.is_empty()
            && self.label_remove.is_empty()
            && self.block.is_empty()
            && self.unblock.is_empty()
    }
}

/// Change fields of a task. One event per change, all in one transaction.
/// (Setting a field to the value it already has is recorded but changes nothing.)
pub fn update(store: &Store, ctx: &Ctx, id: &str, changes: Changes) -> Result<Task> {
    if changes.is_empty() {
        return Err(Error::Usage(
            "nothing to update — pass at least one change (see `hippo-task update --help`)".into(),
        ));
    }
    if changes.assignee.is_some() && changes.unassign {
        return Err(Error::Usage(
            "--assignee and --unassign can't be combined".into(),
        ));
    }
    let title = optional("title", changes.title.as_deref())?;
    let assignee = optional("assignee", changes.assignee.as_deref())?;
    let body = optional("description", changes.body.as_deref())?;
    let label_add = changes
        .label_add
        .iter()
        .map(|l| required("label", l))
        .collect::<Result<Vec<_>>>()?;
    let label_remove = changes
        .label_remove
        .iter()
        .map(|l| required("label", l))
        .collect::<Result<Vec<_>>>()?;

    let mut tx = store.begin()?;
    let proj = fold::fold(tx.events());
    let task = resolve(&proj.tasks, id)?.clone();
    if let Some(state) = changes.state {
        let action = if state.is_closed() {
            "close it"
        } else {
            "change its state"
        };
        guard_holder(&task, ctx, changes.force, action)?;
    }
    let block = targets(&proj.tasks, &task, &changes.block)?;
    let unblock = targets(&proj.tasks, &task, &changes.unblock)?;

    let mut kinds = Vec::new();
    if let Some(title) = title {
        kinds.push(EventKind::SetTitle { title });
    }
    if let Some(state) = changes.state {
        kinds.push(EventKind::SetState { state });
    }
    if let Some(priority) = changes.priority {
        kinds.push(EventKind::SetPriority { priority });
    }
    if assignee.is_some() || changes.unassign {
        kinds.push(EventKind::SetAssignee { assignee });
    }
    if let Some(body) = body {
        kinds.push(EventKind::SetBody { body });
    }
    kinds.extend(
        label_add
            .into_iter()
            .map(|label| EventKind::LabelAdd { label }),
    );
    kinds.extend(
        label_remove
            .into_iter()
            .map(|label| EventKind::LabelRemove { label }),
    );
    kinds.extend(block.into_iter().map(|task| EventKind::Relate {
        rel: RelType::BlockedBy,
        task,
    }));
    kinds.extend(unblock.into_iter().map(|task| EventKind::Unrelate {
        rel: RelType::BlockedBy,
        task,
    }));

    let events: Vec<Event> = kinds
        .into_iter()
        .map(|kind| ctx.event(&mut tx, &task.id, kind))
        .collect();
    let ledger = tx.commit(events)?;
    task_in(fold::fold(&ledger.events), &task.id)
}

// ------------------------------------------------------- lease / start

/// Take (or renew) the execution lease. `Err(Conflict)` if another worker
/// holds it or the task is closed — the attempt is still recorded (contention
/// is part of the audit trail).
pub fn lease(store: &Store, ctx: &Ctx, id: &str, minutes: i64) -> Result<Task> {
    let minutes = lease_minutes(minutes)?;
    let mut tx = store.begin()?;
    let task_id = resolve(&fold::fold(tx.events()).tasks, id)?.id.clone();
    let attempt = ctx.lease_event(&mut tx, &task_id, minutes);
    let ledger = tx.commit(vec![attempt])?;
    let after = task_in(fold::fold(&ledger.events), &task_id)?;
    holds(&after, ctx)?;
    Ok(after)
}

/// Lease + set state to doing, in one transaction. If the lease is refused,
/// only the (rejected) attempt is recorded and the state is left alone.
pub fn start(store: &Store, ctx: &Ctx, id: &str, minutes: i64) -> Result<Task> {
    let minutes = lease_minutes(minutes)?;
    let mut tx = store.begin()?;
    let task_id = resolve(&fold::fold(tx.events()).tasks, id)?.id.clone();
    let attempt = ctx.lease_event(&mut tx, &task_id, minutes);

    // Would the lease be granted? Ask the fold itself — one source of truth.
    let preview = task_in(fold::fold(tx.events().iter().chain([&attempt])), &task_id)?;
    if let Err(conflict) = holds(&preview, ctx) {
        tx.commit(vec![attempt])?;
        return Err(conflict);
    }
    let doing = ctx.event(
        &mut tx,
        &task_id,
        EventKind::SetState {
            state: State::Doing,
        },
    );
    let ledger = tx.commit(vec![attempt, doing])?;
    task_in(fold::fold(&ledger.events), &task_id)
}

/// What [`release`] did.
#[derive(Debug, Clone)]
pub struct Released {
    pub task: Task,
    /// false = you didn't hold the lease, so nothing changed (not an error:
    /// afterwards you don't hold it either way).
    pub released: bool,
}

/// Give back a lease you hold, without completing the task.
pub fn release(store: &Store, ctx: &Ctx, id: &str) -> Result<Released> {
    let mut tx = store.begin()?;
    let proj = fold::fold(tx.events());
    let task = resolve(&proj.tasks, id)?;
    let released = task
        .lease
        .as_ref()
        .is_some_and(|l| l.is_held_by(&ctx.actor, &ctx.node));
    let task_id = task.id.clone();
    let event = ctx.event(&mut tx, &task_id, EventKind::Release);
    let ledger = tx.commit(vec![event])?;
    let task = task_in(fold::fold(&ledger.events), &task_id)?;
    Ok(Released { task, released })
}

// --------------------------------------------------------- done / note

/// Complete a task (and clear its lease). Refused while another worker holds
/// an active lease, unless `force`.
pub fn done(store: &Store, ctx: &Ctx, id: &str, force: bool) -> Result<Task> {
    let mut tx = store.begin()?;
    let task = resolve(&fold::fold(tx.events()).tasks, id)?.clone();
    guard_holder(&task, ctx, force, "close it")?;
    let event = ctx.event(&mut tx, &task.id, EventKind::Complete);
    let ledger = tx.commit(vec![event])?;
    task_in(fold::fold(&ledger.events), &task.id)
}

/// Append a note to the task's history.
pub fn note(store: &Store, ctx: &Ctx, id: &str, text: &str) -> Result<Task> {
    let text = required("note", text)?;
    append_one(store, ctx, id, EventKind::Note { text })
}

/// Set the task's description.
pub fn describe(store: &Store, ctx: &Ctx, id: &str, text: &str) -> Result<Task> {
    let body = required("description", text)?;
    append_one(store, ctx, id, EventKind::SetBody { body })
}

fn append_one(store: &Store, ctx: &Ctx, id: &str, kind: EventKind) -> Result<Task> {
    let mut tx = store.begin()?;
    let task_id = resolve(&fold::fold(tx.events()).tasks, id)?.id.clone();
    let event = ctx.event(&mut tx, &task_id, kind);
    let ledger = tx.commit(vec![event])?;
    task_in(fold::fold(&ledger.events), &task_id)
}

// ------------------------------------------------------------ list/show

/// Order for [`list`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Sort {
    /// By task number (creation order).
    #[default]
    Num,
    /// Most urgent first.
    Priority,
    /// Oldest first.
    Created,
    /// Most recently changed first.
    Updated,
}

/// Filters for [`list`]. The default lists every task by number.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub state: Option<State>,
    /// Only tasks whose *active* lease this worker (actor + node) holds.
    pub mine: bool,
    pub blocked: bool,
    pub sort: Sort,
}

/// The tasks matching `filter`, in `filter.sort` order.
pub fn list(store: &Store, ctx: &Ctx, filter: &Filter) -> Result<Vec<Task>> {
    let ledger = store.read()?;
    let mut tasks: Vec<Task> = fold::fold(&ledger.events)
        .tasks
        .into_values()
        .filter(|t| filter.state.is_none_or(|s| t.state == s))
        .filter(|t| {
            !filter.mine
                || t.lease
                    .as_ref()
                    .is_some_and(|l| l.is_active(ctx.now_ms) && l.is_held_by(&ctx.actor, &ctx.node))
        })
        .filter(|t| !filter.blocked || t.blocked)
        .collect();
    match filter.sort {
        Sort::Num => tasks.sort_by_key(|t| t.num),
        Sort::Priority => tasks.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.num.cmp(&b.num))),
        Sort::Created => tasks.sort_by_key(|t| (t.created_ms, t.num)),
        Sort::Updated => {
            tasks.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms).then(a.num.cmp(&b.num)))
        }
    }
    Ok(tasks)
}

/// One history entry: the event, and whether it changed anything.
#[derive(Debug, Clone)]
pub struct Entry {
    pub event: Event,
    /// false = recorded but no effect (a rejected lease, a repeat, …).
    pub applied: bool,
}

/// Everything `show` needs.
#[derive(Debug, Clone)]
pub struct Detail {
    pub task: Task,
    /// This task's events, in fold order.
    pub history: Vec<Entry>,
    /// Every task, for describing references (`blocked-by #2 "Write docs"`).
    pub all: BTreeMap<String, Task>,
}

/// One task plus its full, annotated history.
pub fn show(store: &Store, id: &str) -> Result<Detail> {
    let ledger = store.read()?;
    let proj = fold::fold(&ledger.events);
    let task = resolve(&proj.tasks, id)?.clone();
    let history = fold::in_order(&ledger.events)
        .into_iter()
        .filter(|e| e.task == task.id)
        .map(|e| Entry {
            event: e.clone(),
            applied: !proj.noops.contains(&e.eid),
        })
        .collect();
    Ok(Detail {
        task,
        history,
        all: proj.tasks,
    })
}

// -------------------------------------------------------------- helpers

/// Resolve what the user typed into a task:
/// - digits (`12` or `#12`) → the task *number* — never a suffix match;
/// - a full id (any case — ULIDs are case-insensitive);
/// - or a unique suffix of at least `MIN_SUFFIX` characters.
fn resolve<'a>(tasks: &'a BTreeMap<String, Task>, input: &str) -> Result<&'a Task> {
    let s = input.trim();
    let digits = s.strip_prefix('#').unwrap_or(s);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        let num: Option<u64> = digits.parse().ok();
        return num
            .and_then(|n| tasks.values().find(|t| t.num == n))
            .ok_or_else(|| Error::NotFound(input.to_string()));
    }
    let wanted = s.to_ascii_uppercase();
    if let Some(t) = tasks.get(s).or_else(|| tasks.get(&wanted)) {
        return Ok(t);
    }
    if s.chars().count() < MIN_SUFFIX {
        return Err(Error::Usage(format!(
            "'{input}' is too short to identify a task — use its number (e.g. 3) or at least {MIN_SUFFIX} characters of its id"
        )));
    }
    let matches: Vec<&Task> = tasks
        .values()
        .filter(|t| t.id.to_ascii_uppercase().ends_with(&wanted))
        .collect();
    match matches.as_slice() {
        [one] => Ok(*one),
        [] => Err(Error::NotFound(input.to_string())),
        many => {
            let handles: Vec<String> = many.iter().map(|t| t.handle()).collect();
            Err(Error::Usage(format!(
                "'{input}' matches {} tasks ({}) — use more characters, or the number",
                many.len(),
                handles.join(", ")
            )))
        }
    }
}

/// Resolve `--block` / `--unblock` targets; a task can't block itself.
fn targets(tasks: &BTreeMap<String, Task>, task: &Task, inputs: &[String]) -> Result<Vec<String>> {
    inputs
        .iter()
        .map(|input| {
            let target = resolve(tasks, input)?;
            if target.id == task.id {
                return Err(Error::Usage(format!(
                    "{} can't be blocked by itself",
                    task.handle()
                )));
            }
            Ok(target.id.clone())
        })
        .collect()
}

/// Etiquette: while another worker holds an active lease, the task's *state*
/// is theirs — only they may change it or close it. Metadata (title, priority,
/// labels, notes…) stays collaborative. `force` overrides (e.g. a human
/// cleaning up after a crashed agent).
fn guard_holder(task: &Task, ctx: &Ctx, force: bool, action: &str) -> Result<()> {
    match &task.lease {
        Some(l) if !force && l.is_active(ctx.now_ms) && !l.is_held_by(&ctx.actor, &ctx.node) => {
            Err(Error::Conflict(format!(
                "{} is leased to {} ({}) — only the holder can {action}; wait for it to finish or expire, or pass --force",
                task.handle(),
                who(&l.holder, &l.node),
                remaining(l.expires_ms, ctx.now_ms)
            )))
        }
        _ => Ok(()),
    }
}

/// After a lease attempt: do *we* (this actor on this node) hold it?
fn holds(task: &Task, ctx: &Ctx) -> Result<()> {
    match &task.lease {
        Some(l) if l.is_held_by(&ctx.actor, &ctx.node) => Ok(()),
        Some(l) => Err(Error::Conflict(format!(
            "{} is leased to {} ({}) — back off and pick another task",
            task.handle(),
            who(&l.holder, &l.node),
            remaining(l.expires_ms, ctx.now_ms)
        ))),
        None if task.state.is_closed() => Err(Error::Conflict(format!(
            "{} is {} — it can't be leased; reopen it first: hippo-task update {} --state todo",
            task.handle(),
            task.state,
            task.num
        ))),
        None => Err(Error::Conflict(format!(
            "{}: the lease was not granted",
            task.handle()
        ))),
    }
}

fn lease_minutes(minutes: i64) -> Result<i64> {
    if (1..=MAX_LEASE_MINUTES).contains(&minutes) {
        Ok(minutes)
    } else {
        Err(Error::Usage(format!(
            "--minutes must be between 1 and {MAX_LEASE_MINUTES} (renew to hold a task longer)"
        )))
    }
}

/// A required text field, trimmed; empty is a usage error.
fn required(what: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(Error::Usage(format!("{what} must not be empty")))
    } else {
        Ok(trimmed.to_string())
    }
}

/// An optional text field: absent is fine, present-but-empty is not.
fn optional(what: &str, value: Option<&str>) -> Result<Option<String>> {
    value.map(|v| required(what, v)).transpose()
}

/// Pull one task out of a projection (by exact id).
fn task_in(mut proj: Projection, id: &str) -> Result<Task> {
    proj.tasks
        .remove(id)
        .ok_or_else(|| Error::NotFound(id.to_string()))
}
