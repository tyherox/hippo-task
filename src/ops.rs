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

use crate::config::{Config, Field};
use crate::error::{Error, Result};
use crate::fold::{self, Projection};
use crate::media::{self, Anchor};
use crate::model::{Event, EventKind, Lease, Priority, RelType, State, Task};
use crate::render::{self, hold_status, who};
use crate::store::{Store, Tx};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use ulid::Ulid;

/// Shortest id suffix accepted, so a stray `hippo-task done A` can't hit a random task.
const MIN_SUFFIX: usize = 4;

/// Who is acting, and when. Built once per command by the CLI.
#[derive(Debug, Clone)]
pub struct Ctx {
    /// Semantic who: `human:local`, `agent:claude`, …
    pub actor: String,
    /// Which writer instance (window / session). A claim belongs to actor + node.
    pub node: String,
    /// Wall-clock "now" in unix millis.
    pub now_ms: i64,
}

impl Ctx {
    /// Who is acting. Privacy by default: without `--actor`/`HIPPO_ACTOR` the
    /// ledger records `human:local` — never `$USER`, a hostname, or anything else
    /// taken from the machine. (Guarded by a test in tests/cli.rs.)
    ///
    /// Agents must name their node: a claim belongs to actor + node, so if every
    /// window of `agent:claude` defaulted to the same node they'd be one worker,
    /// and the claim would no longer stop two windows taking the same task.
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
    /// Declared fields to set, as `(name, value)` (ADR-006; see [`parse_field`]).
    pub fields: Vec<(String, String)>,
}

/// Create a task (plus one event per label and per field, in the same transaction).
pub fn add(store: &Store, ctx: &Ctx, new: NewTask) -> Result<Task> {
    let title = required("title", &new.title)?;
    let labels = new
        .labels
        .iter()
        .map(|l| required("label", l))
        .collect::<Result<Vec<_>>>()?;
    let assignee = optional("assignee", new.assignee.as_deref())?;
    // Images are copied before the ledger is locked, so a large file doesn't
    // hold up other writers (ADR-008).
    let body = match new.body.as_deref() {
        Some(text) => Some(stored_body(store, text, &Anchor::Cwd)?),
        None => None,
    };
    let fields = if new.fields.is_empty() {
        Vec::new() // no fields, no need to read the config
    } else {
        checked_fields(&store.config()?, &new.fields)?
    };

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
    for (field, value) in fields {
        let set = EventKind::SetField {
            field,
            value: Some(value),
        };
        events.push(ctx.event(&mut tx, &id, set));
    }
    let ledger = tx.commit(events)?;
    task_in(fold::fold(&ledger.events), &id)
}

/// One `--field` argument, `name=value`, split and trimmed.
///
/// Rust note: `split_once` splits at the *first* `=`, so a value may contain
/// one itself (`--field formula=a=b`).
pub fn parse_field(arg: &str) -> Result<(String, String)> {
    let Some((name, value)) = arg.split_once('=') else {
        return Err(Error::Usage(format!(
            "--field takes name=value, like project=dashboard — got `{arg}`"
        )));
    };
    let (name, value) = (name.trim(), value.trim());
    if name.is_empty() {
        return Err(Error::Usage(format!(
            "`{arg}` names no field — --field takes name=value, like project=dashboard"
        )));
    }
    if value.is_empty() {
        return Err(Error::Usage(format!(
            "`{arg}` has no value — to clear a field, use --clear-field {name}"
        )));
    }
    Ok((name.to_string(), value.to_string()))
}

/// Fields to set, checked against the config (ADR-006): each one declared,
/// each value allowed, and none set twice — a field holds one value.
fn checked_fields(config: &Config, fields: &[(String, String)]) -> Result<Vec<(String, String)>> {
    let mut seen = BTreeSet::new();
    for (name, value) in fields {
        if !seen.insert(name.as_str()) {
            return Err(Error::Usage(format!(
                "{name} is set twice — a field holds one value"
            )));
        }
        config.check(name, value)?;
    }
    Ok(fields.to_vec())
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
    /// Mark this task a duplicate of another (ADR-004): link it to the
    /// original and cancel it. Closing etiquette applies, as for `--state`.
    pub duplicate_of: Option<String>,
    /// Declared fields to set, as `(name, value)` (ADR-006).
    pub fields: Vec<(String, String)>,
    /// Fields to clear, by name.
    pub clear_fields: Vec<String>,
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
            && self.duplicate_of.is_none()
            && self.fields.is_empty()
            && self.clear_fields.is_empty()
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
    if changes.duplicate_of.is_some() && changes.state.is_some() {
        return Err(Error::Usage(
            "--duplicate-of already cancels the task — don't combine it with --state".into(),
        ));
    }
    let title = optional("title", changes.title.as_deref())?;
    let assignee = optional("assignee", changes.assignee.as_deref())?;
    let body = match changes.body.as_deref() {
        Some(text) => Some(stored_body(store, text, &Anchor::Cwd)?),
        None => None,
    };
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
    let clear_fields = changes
        .clear_fields
        .iter()
        .map(|f| required("field", f))
        .collect::<Result<Vec<_>>>()?;
    let config = if changes.fields.is_empty() && clear_fields.is_empty() {
        None // no fields involved, no need to read the config
    } else {
        Some(store.config()?)
    };
    let set_fields = match &config {
        Some(config) if !changes.fields.is_empty() => checked_fields(config, &changes.fields)?,
        _ => Vec::new(),
    };
    if let Some(name) = clear_fields
        .iter()
        .find(|name| set_fields.iter().any(|(set, _)| set == *name))
    {
        return Err(Error::Usage(format!(
            "{name} is both set and cleared — pick one"
        )));
    }

    let mut tx = store.begin()?;
    let proj = fold::fold(tx.events());
    let task = resolve(&proj.tasks, id)?.clone();
    // Clearing works on any field the task carries — even one the config no
    // longer declares, so old values can be cleaned up. Otherwise the name
    // must be declared: a typo mustn't silently clear nothing.
    if let Some(config) = &config {
        for name in &clear_fields {
            if !task.fields.contains_key(name) {
                config.declared(name)?;
            }
        }
    }
    if let Some(state) = changes.state {
        let action = if state.is_closed() {
            "close it"
        } else {
            "change its state"
        };
        guard_holder(&task, ctx, changes.force, action)?;
    }
    // Marking a duplicate closes the task, so the same etiquette applies.
    if changes.duplicate_of.is_some() {
        guard_holder(&task, ctx, changes.force, "close it")?;
    }
    let block = targets(&proj.tasks, &task, &changes.block)?;
    let unblock = targets(&proj.tasks, &task, &changes.unblock)?;
    let original = match &changes.duplicate_of {
        Some(input) => Some(original_of(&proj.tasks, &task, input)?),
        None => None,
    };

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
    kinds.extend(
        set_fields
            .into_iter()
            .map(|(field, value)| EventKind::SetField {
                field,
                value: Some(value),
            }),
    );
    kinds.extend(
        clear_fields
            .into_iter()
            .map(|field| EventKind::SetField { field, value: None }),
    );
    kinds.extend(block.into_iter().map(|task| EventKind::Relate {
        rel: RelType::BlockedBy,
        task,
    }));
    kinds.extend(unblock.into_iter().map(|task| EventKind::Unrelate {
        rel: RelType::BlockedBy,
        task,
    }));
    // ADR-004: link to the original, then cancel — a duplicate isn't finished work.
    if let Some(original) = original {
        kinds.push(EventKind::Relate {
            rel: RelType::DuplicateOf,
            task: original,
        });
        kinds.push(EventKind::SetState {
            state: State::Cancelled,
        });
    }

    let events: Vec<Event> = kinds
        .into_iter()
        .map(|kind| ctx.event(&mut tx, &task.id, kind))
        .collect();
    let ledger = tx.commit(events)?;
    task_in(fold::fold(&ledger.events), &task.id)
}

// ------------------------------------------------------ start / release

/// Claim a task and set it to doing, in one transaction. The claim has no
/// timer (ADR-003): it's yours until you finish or release it, or someone
/// reclaims it. `Err(Conflict)` if another worker holds it or the task is
/// closed — and then only the (rejected) attempt is recorded, because
/// contention is part of the audit trail.
pub fn start(store: &Store, ctx: &Ctx, id: &str) -> Result<Task> {
    let mut tx = store.begin()?;
    let task_id = resolve(&fold::fold(tx.events()).tasks, id)?.id.clone();
    let claim = EventKind::Claim {
        holder: ctx.actor.clone(),
    };
    let attempt = ctx.event(&mut tx, &task_id, claim);

    // Would the claim be granted? Ask the fold itself — one source of truth.
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
    task_in(fold::fold(&ledger.events), &task_id).map(|task| present(store, task))
}

/// What [`release`] did.
#[derive(Debug, Clone)]
pub struct Released {
    pub task: Task,
    /// false = you didn't hold the lease, so nothing changed (not an error:
    /// afterwards you don't hold it either way).
    pub released: bool,
}

/// Give back a lease you hold, without completing the task. If you had
/// started it (state `doing`), it goes back to `todo` for the next worker:
/// `release` undoes `start` (ADR-002).
pub fn release(store: &Store, ctx: &Ctx, id: &str) -> Result<Released> {
    let mut tx = store.begin()?;
    let proj = fold::fold(tx.events());
    let task = resolve(&proj.tasks, id)?;
    let released = task
        .lease
        .as_ref()
        .is_some_and(|l| l.is_held_by(&ctx.actor, &ctx.node));
    let back_to_todo = released && task.state == State::Doing;
    let task_id = task.id.clone();

    // Undo `start` in reverse order: reset the state while you still hold the
    // lease (so the change is yours to make), then let go. Only the release is
    // recorded when there's no state to reset — or when it isn't your lease,
    // in which case the fold records it as a no-op.
    let mut events = Vec::with_capacity(2);
    if back_to_todo {
        let todo = EventKind::SetState { state: State::Todo };
        events.push(ctx.event(&mut tx, &task_id, todo));
    }
    events.push(ctx.event(&mut tx, &task_id, EventKind::Release));
    let ledger = tx.commit(events)?;
    let task = present(store, task_in(fold::fold(&ledger.events), &task_id)?);
    Ok(Released { task, released })
}

/// Give back everything this worker (actor + node) holds, in one transaction —
/// for a session-end hook or a workflow's teardown (ADR-003). Each `doing` task
/// goes back to `todo`, as with [`release`]. Returns the tasks given back.
pub fn release_all(store: &Store, ctx: &Ctx) -> Result<Vec<Task>> {
    let mut tx = store.begin()?;
    let mut held: Vec<Task> = fold::fold(tx.events())
        .tasks
        .into_values()
        .filter(|t| {
            t.lease
                .as_ref()
                .is_some_and(|l| l.is_held_by(&ctx.actor, &ctx.node))
        })
        .collect();
    if held.is_empty() {
        return Ok(held); // nothing to record — dropping `tx` releases the lock
    }
    held.sort_by_key(|t| t.num);
    let mut events = Vec::new();
    for t in &held {
        // As in `release`: reset the state while still holding it, then let go.
        if t.state == State::Doing {
            let todo = EventKind::SetState { state: State::Todo };
            events.push(ctx.event(&mut tx, &t.id, todo));
        }
        events.push(ctx.event(&mut tx, &t.id, EventKind::Release));
    }
    let ledger = tx.commit(events)?;
    tasks_in(fold::fold(&ledger.events), &held).map(|tasks| present_all(store, tasks))
}

/// What [`reclaim`] takes back.
#[derive(Debug, Clone)]
pub enum ReclaimTarget {
    /// One task, from whoever holds it.
    Task(String),
    /// Everything held on one node — a worker's window or session.
    Node(String),
}

/// What [`reclaim`] took back: the task as it is now, and whose hold it was.
#[derive(Debug, Clone)]
pub struct Reclaimed {
    pub task: Task,
    pub from: Lease,
}

/// Take back work from a worker that can't give it back itself (ADR-003): a
/// person, or the orchestrator that launched it, knows it's gone. Each
/// `reclaim` event names the hold it takes back, so a stale one can't clobber
/// a newer claim, and each `doing` task goes back to `todo`.
///
/// Etiquette: human actors may reclaim; an agent needs `force`, which
/// AGENTS.md reserves for the process that launched the worker. Returns what
/// was taken back — nothing if nobody held it, which isn't an error.
pub fn reclaim(
    store: &Store,
    ctx: &Ctx,
    target: &ReclaimTarget,
    reason: Option<&str>,
    force: bool,
) -> Result<Vec<Reclaimed>> {
    let reason = optional("reason", reason)?;
    if !force && !ctx.actor.starts_with("human:") {
        return Err(Error::Conflict(
            "agents need --force to reclaim — AGENTS.md reserves it for the process that launched the worker (an orchestrator)".into(),
        ));
    }
    let mut tx = store.begin()?;
    let proj = fold::fold(tx.events());
    // Each held task, paired with the hold it has now.
    let mut held: Vec<(Task, Lease)> = match target {
        ReclaimTarget::Task(id) => {
            let t = resolve(&proj.tasks, id)?;
            match &t.lease {
                Some(l) => vec![(t.clone(), l.clone())],
                None => Vec::new(),
            }
        }
        ReclaimTarget::Node(node) => proj
            .tasks
            .into_values()
            .filter(|t| t.lease.as_ref().is_some_and(|l| l.node == *node))
            .filter_map(|t| {
                let l = t.lease.clone()?;
                Some((t, l))
            })
            .collect(),
    };
    if held.is_empty() {
        return Ok(Vec::new()); // nobody held it — nothing to record
    }
    held.sort_by_key(|(t, _)| t.num);
    let mut events = Vec::new();
    for (t, l) in &held {
        let take_back = EventKind::Reclaim {
            holder: l.holder.clone(),
            node: l.node.clone(),
            reason: reason.clone(),
        };
        events.push(ctx.event(&mut tx, &t.id, take_back));
        // With the hold gone, the state is anyone's to change: back to the queue.
        if t.state == State::Doing {
            let todo = EventKind::SetState { state: State::Todo };
            events.push(ctx.event(&mut tx, &t.id, todo));
        }
    }
    let ledger = tx.commit(events)?;
    let mut after = fold::fold(&ledger.events);
    held.into_iter()
        .map(|(t, from)| {
            let task = after
                .tasks
                .remove(&t.id)
                .ok_or_else(|| Error::NotFound(t.id.clone()))?;
            media::warn_about(store, std::slice::from_ref(&task), false);
            Ok(Reclaimed { task, from })
        })
        .collect()
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
    task_in(fold::fold(&ledger.events), &task.id).map(|task| present(store, task))
}

/// Append a note to the task's history.
pub fn note(store: &Store, ctx: &Ctx, id: &str, text: &str) -> Result<Task> {
    let text = required("note", text)?;
    append_one(store, ctx, id, EventKind::Note { text })
}

/// Set the task's description (ADR-008). `anchor` is where relative image
/// paths resolve. `base` is the task's `seq` from the caller's last read:
/// when the description has moved on, the three versions merge by paragraph.
/// Without `base`, the text replaces the description. No holder check —
/// the description stays collaborative, like the title.
pub fn describe(
    store: &Store,
    ctx: &Ctx,
    id: &str,
    markdown: &str,
    anchor: &Anchor,
    base: Option<u64>,
) -> Result<Task> {
    let submitted = stored_body(store, markdown, anchor)?;
    let mut tx = store.begin()?;
    let task = resolve(&fold::fold(tx.events()).tasks, id)?.clone();
    let body = match base {
        None => submitted,
        Some(base_seq) => merged_body(tx.events(), &task, base_seq, submitted)?,
    };
    let event = ctx.event(&mut tx, &task.id, EventKind::SetBody { body });
    let ledger = tx.commit(vec![event])?;
    task_in(fold::fold(&ledger.events), &task.id)
}

/// Normalize, copy images, and refuse an empty result. The config read is
/// what supplies the size caps; it does not take the ledger lock.
fn stored_body(store: &Store, markdown: &str, anchor: &Anchor) -> Result<String> {
    let limits = store.config()?.media;
    finish_body(media::ingest(store, markdown, anchor, &limits)?)
}

fn finish_body(text: String) -> Result<String> {
    if text.trim().is_empty() {
        Err(Error::Usage("description must not be empty".into()))
    } else {
        Ok(text)
    }
}

fn merged_body(events: &[Event], task: &Task, base: u64, submitted: String) -> Result<String> {
    if base > task.seq {
        return Err(Error::Usage(format!(
            "seq {base} is past {}'s current seq {} — re-read the task and pass its seq",
            task.handle(),
            task.seq
        )));
    }
    if base == task.seq {
        return Ok(submitted);
    }
    let historical = fold::body_at_seq(events, &task.id, base).ok_or_else(|| {
        Error::Usage(format!(
            "seq {base} doesn't name a description of {} (a task starts at seq 1; this one is at seq {})",
            task.handle(),
            task.seq
        ))
    })?;
    let base_paras = media::paragraphs(historical.as_deref().unwrap_or(""));
    let ours = media::paragraphs(&submitted);
    let theirs = media::paragraphs(task.body.as_deref().unwrap_or(""));
    match media::merge(&base_paras, &ours, &theirs) {
        Ok(paras) => finish_body(paras.join("\n\n")),
        Err(conflicts) => Err(Error::Stale(stale_message(task, base, &conflicts))),
    }
}

fn stale_message(task: &Task, base: u64, conflicts: &[media::Conflict]) -> String {
    let mut msg = format!(
        "{}'s description changed since seq {base} — these paragraphs conflict, so nothing was written:",
        task.handle()
    );
    for conflict in conflicts {
        msg.push_str(&format!(
            "\n  submitted: \"{}\"\n  current: \"{}\"",
            clip_conflict(&conflict.submitted),
            clip_conflict(&conflict.current)
        ));
    }
    msg.push_str(&format!(
        "\nre-read the task (hippo-task show {}) and redo the edit",
        task.num
    ));
    msg
}

fn clip_conflict(text: &str) -> String {
    let flat = text.replace('\n', " ");
    let mut chars = flat.chars();
    let short: String = chars.by_ref().take(80).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}

fn append_one(store: &Store, ctx: &Ctx, id: &str, kind: EventKind) -> Result<Task> {
    let mut tx = store.begin()?;
    let task_id = resolve(&fold::fold(tx.events()).tasks, id)?.id.clone();
    let event = ctx.event(&mut tx, &task_id, kind);
    let ledger = tx.commit(vec![event])?;
    task_in(fold::fold(&ledger.events), &task_id).map(|task| present(store, task))
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
    /// Only tasks someone could pick up right now: open, not blocked, and no
    /// active hold (so this includes `doing` work whose 0.1.x lease ran out — ADR-002).
    pub ready: bool,
    /// Only tasks someone holds right now; each hold says when its holder was
    /// last seen (ADR-003).
    pub held: bool,
    /// Only tasks whose title or description contains every one of these,
    /// ignoring case — the check before filing a new task (ADR-004).
    pub search: Vec<String>,
    /// Only tasks with every one of these field values, as `(name, value)` (ADR-006).
    pub fields: Vec<(String, String)>,
    /// Only tasks with every one of these labels.
    pub labels: Vec<String>,
    pub sort: Sort,
}

/// The tasks matching `filter`, in `filter.sort` order.
pub fn list(store: &Store, ctx: &Ctx, filter: &Filter) -> Result<Vec<Task>> {
    // Lowercase once here, so matching below is a plain `contains`.
    let terms = filter
        .search
        .iter()
        .map(|term| required("search text", term).map(|t| t.to_lowercase()))
        .collect::<Result<Vec<_>>>()?;
    let config = if filter.fields.is_empty() {
        Config::default() // no field filters, no need to read the config
    } else {
        store.config()?
    };
    let labels = checked_filters(store, &config, &filter.fields, &filter.labels)?;
    let ledger = store.read()?;
    let mut tasks: Vec<Task> = fold::fold(&ledger.events)
        .tasks
        .into_values()
        .filter(|t| terms.iter().all(|term| mentions(t, term)))
        .filter(|t| carries(t, &filter.fields, &labels))
        .filter(|t| filter.state.is_none_or(|s| t.state == s))
        .filter(|t| {
            !filter.mine
                || t.lease
                    .as_ref()
                    .is_some_and(|l| l.is_active(ctx.now_ms) && l.is_held_by(&ctx.actor, &ctx.node))
        })
        .filter(|t| !filter.blocked || t.blocked)
        .filter(|t| !filter.ready || is_ready(t, ctx.now_ms))
        .filter(|t| !filter.held || t.lease.as_ref().is_some_and(|l| l.is_active(ctx.now_ms)))
        .collect();
    match filter.sort {
        Sort::Num => tasks.sort_by_key(|t| t.num),
        Sort::Priority => tasks.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.num.cmp(&b.num))),
        Sort::Created => tasks.sort_by_key(|t| (t.created_ms, t.num)),
        Sort::Updated => {
            tasks.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms).then(a.num.cmp(&b.num)))
        }
    }
    // Present only: list doesn't re-hash. A missing file is a warning.
    media::warn_about(store, &tasks, false);
    Ok(tasks)
}

/// Check `--field` and `--label` filters; returns the labels, trimmed. A
/// field must be declared, and named once — it holds one value, so two could
/// never both match. A value off its list still filters — old tasks may carry
/// it — but says so as a warning, so a typo doesn't look like "no tasks".
fn checked_filters(
    store: &Store,
    config: &Config,
    fields: &[(String, String)],
    labels: &[String],
) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    for (name, value) in fields {
        if !seen.insert(name.as_str()) {
            return Err(Error::Usage(format!(
                "{name} is filtered twice — a field holds one value, so no task could match both"
            )));
        }
        if let Some(warning) = config.check_filter(name, value)? {
            store.warn(&warning);
        }
    }
    labels.iter().map(|l| required("label", l)).collect()
}

/// Does the task carry every one of these field values and labels?
fn carries(t: &Task, fields: &[(String, String)], labels: &[String]) -> bool {
    fields
        .iter()
        .all(|(name, value)| t.fields.get(name) == Some(value))
        && labels.iter().all(|label| t.labels.contains(label))
}

/// Does the task's title or description contain `term` (already lowercased)?
/// A plain substring, not a pattern: what an agent types is what it finds.
/// Image destinations are skipped, so `media` doesn't match every file; the
/// caption and the prose still match (ADR-008).
fn mentions(t: &Task, term: &str) -> bool {
    t.title.to_lowercase().contains(term)
        || t.body
            .as_deref()
            .is_some_and(|body| media::searchable(body).contains(term))
}

/// Ready to pick up: open, not blocked, and nobody holds it. That includes
/// `doing` work whose 0.1.x lease ran out: the fold can't move it back to
/// `todo`, because time passing isn't an event. (A claim never runs out.)
fn is_ready(t: &Task, now_ms: i64) -> bool {
    let held = t.lease.as_ref().is_some_and(|l| l.is_active(now_ms));
    !t.state.is_closed() && !t.blocked && !held
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
    // show re-reads each file and warns when the bytes don't match the name.
    media::warn_about(store, std::slice::from_ref(&task), true);
    Ok(Detail {
        task,
        history,
        all: proj.tasks,
    })
}

// --------------------------------------------------------------- fields

/// How many tasks carry one value of a field: the open ones, and all of them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValueCount {
    pub value: String,
    pub open: usize,
    pub total: usize,
}

/// A value a task carries that the config doesn't allow — any more, usually.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stray {
    pub num: u64,
    pub id: String,
    pub field: String,
    pub value: String,
}

/// What `hippo-task fields` reports (ADR-006).
#[derive(Debug, Clone)]
pub struct FieldsReport {
    pub config: Config,
    /// For each declared field, in `config.fields` order: its values and their
    /// use — every allowed value for a listed field, the values in use otherwise.
    pub counts: Vec<Vec<ValueCount>>,
    pub strays: Vec<Stray>,
}

/// The declared fields, how much each value is used, and any strays.
pub fn fields(store: &Store) -> Result<FieldsReport> {
    let config = store.config()?;
    let tasks: Vec<Task> = fold::fold(&store.read()?.events)
        .tasks
        .into_values()
        .collect();
    let counts = config
        .fields
        .iter()
        .map(|f| {
            let values: Vec<String> = match &f.values {
                Some(list) => list.clone(),
                // Any text: the distinct values in use, sorted (a BTreeSet does both).
                None => tasks
                    .iter()
                    .filter_map(|t| t.fields.get(&f.name).cloned())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            };
            values
                .into_iter()
                .map(|value| {
                    let carrying: Vec<&Task> = tasks
                        .iter()
                        .filter(|t| t.fields.get(&f.name) == Some(&value))
                        .collect();
                    let open = carrying.iter().filter(|t| !t.state.is_closed()).count();
                    ValueCount {
                        value,
                        open,
                        total: carrying.len(),
                    }
                })
                .collect()
        })
        .collect();
    let mut strays: Vec<Stray> = tasks
        .iter()
        .flat_map(|t| {
            t.fields
                .iter()
                .filter(|(name, value)| match config.field(name) {
                    None => true, // the field isn't declared (any more)
                    Some(f) => f.values.as_ref().is_some_and(|list| !list.contains(value)),
                })
                .map(|(name, value)| Stray {
                    num: t.num,
                    id: t.id.clone(),
                    field: name.clone(),
                    value: value.clone(),
                })
        })
        .collect();
    strays.sort_by(|a, b| a.num.cmp(&b.num).then(a.field.cmp(&b.field)));
    Ok(FieldsReport {
        config,
        counts,
        strays,
    })
}

// --------------------------------------------------------------- export

/// Where an export goes (ADR-007). Notion is the first; others can follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Destination {
    /// CSV for Notion's importer (Import, or Merge with CSV).
    Notion,
}

impl Destination {
    /// The name recorded on export events and in each task's `exported`.
    pub fn as_str(self) -> &'static str {
        match self {
            Destination::Notion => "notion",
        }
    }
}

/// Which tasks an export takes. By default: open tasks not exported there yet.
#[derive(Debug, Clone, Default)]
pub struct ExportSelection {
    /// Only tasks with every one of these field values.
    pub fields: Vec<(String, String)>,
    /// Only tasks with every one of these labels.
    pub labels: Vec<String>,
    /// Closed tasks too.
    pub all: bool,
    /// Tasks already exported there, too.
    pub again: bool,
}

/// What [`export`] did.
#[derive(Debug, Clone)]
pub struct Exported {
    /// The CSV: a header, then one row per task.
    pub csv: String,
    /// The tasks in it, in order — as they are after the export was recorded.
    pub tasks: Vec<Task>,
    /// Tasks left out because they were exported before but changed since —
    /// to fix in the destination by hand, or to export again with `again`.
    pub changed: Vec<Task>,
    /// The file written: `None` for a preview, or when there was nothing new.
    pub file: Option<PathBuf>,
    /// Files this command copied beside the CSV. One already there with the
    /// same bytes is left alone and is not listed (ADR-008).
    pub media_files: Vec<PathBuf>,
}

/// Export tasks for another tool (ADR-007). With `out`, write that file —
/// never over an existing one — and record an `export` event on each task, so
/// the next export skips them. Without `out` it's a preview: nothing recorded.
pub fn export(
    store: &Store,
    ctx: &Ctx,
    to: Destination,
    selection: &ExportSelection,
    out: Option<&Path>,
) -> Result<Exported> {
    let config = store.config()?;
    let labels = checked_filters(store, &config, &selection.fields, &selection.labels)?;
    let Some(out) = out else {
        // A preview: read, choose, render — record nothing.
        let events = store.read()?.events;
        let proj = fold::fold(&events);
        let (picked, changed) = choose(&events, &proj, selection, &labels, to, &config.fields);
        let csv = render::notion_csv(&picked, &proj.tasks, &config.fields);
        media::warn_about(store, &picked, true);
        media::warn_about(store, &changed, true);
        return Ok(Exported {
            csv,
            tasks: picked,
            changed,
            file: None,
            media_files: Vec::new(),
        });
    };
    // For real: choose under the lock, so what's recorded is what was written.
    let mut tx = store.begin()?;
    let proj = fold::fold(tx.events());
    let (picked, changed) = choose(tx.events(), &proj, selection, &labels, to, &config.fields);
    let csv = render::notion_csv(&picked, &proj.tasks, &config.fields);
    media::warn_about(store, &picked, true);
    media::warn_about(store, &changed, true);
    if picked.is_empty() {
        return Ok(Exported {
            csv,
            tasks: picked,
            changed,
            file: None, // nothing new: no empty file to import by mistake
            media_files: Vec::new(),
        });
    }
    // Refuse an existing CSV before copying, so a refusal leaves no new files.
    if out.exists() {
        return Err(Error::Usage(format!(
            "{} already exists — export to a new file (and if that one hasn't been imported yet, import it first)",
            out.display()
        )));
    }
    let markdowns: Vec<&str> = picked.iter().filter_map(|t| t.body.as_deref()).collect();
    let dest_dir = out.parent().unwrap_or(Path::new(".")).join("media");
    let copied = media::copy_referenced(store.folder(), &markdowns, &dest_dir)?;
    // The file first: if it can't be written, nothing is marked exported.
    if let Err(error) = write_new_file(out, &csv) {
        return Err(unrecorded(out, &copied, error));
    }
    let events: Vec<Event> = picked
        .iter()
        .map(|t| {
            let kind = EventKind::Export {
                to: to.as_str().to_string(),
            };
            ctx.event(&mut tx, &t.id, kind)
        })
        .collect();
    let ledger = tx.commit(events).map_err(|e| unrecorded(out, &copied, e))?;
    let tasks = tasks_in(fold::fold(&ledger.events), &picked)?;
    Ok(Exported {
        csv,
        tasks,
        changed,
        file: Some(out.to_path_buf()),
        media_files: copied,
    })
}

/// The tasks to export, and the ones skipped because their row changed after
/// their last export — both in task-number order. `--all` decides only which
/// tasks are exported: a task closed since its export is reported as changed.
fn choose(
    events: &[Event],
    proj: &Projection,
    sel: &ExportSelection,
    labels: &[String],
    to: Destination,
    fields: &[Field],
) -> (Vec<Task>, Vec<Task>) {
    let mut candidates: Vec<&Task> = proj
        .tasks
        .values()
        .filter(|t| carries(t, &sel.fields, labels))
        .collect();
    candidates.sort_by_key(|t| t.num);
    let mut then = AsExported::new(events);
    let mut picked = Vec::new();
    let mut changed = Vec::new();
    for t in candidates {
        let eligible = sel.all || !t.state.is_closed();
        match t.exported.get(to.as_str()) {
            None if eligible => picked.push(t.clone()),
            Some(_) if sel.again && eligible => picked.push(t.clone()),
            Some(&at)
                if then.row(at, &t.id, fields)
                    != Some(render::notion_row(t, &proj.tasks, fields)) =>
            {
                changed.push(t.clone())
            }
            _ => {}
        }
    }
    (picked, changed)
}

/// The ledger as each export found it (ADR-007), to render a task's row as it
/// was uploaded. An export changes no content, so only the other events count —
/// and the tasks of one export batch share one fold.
struct AsExported<'a> {
    /// Every event but exports, in fold order.
    content: Vec<&'a Event>,
    /// Folds of `content[..n]`, keyed by `n`.
    folds: BTreeMap<usize, Projection>,
}

impl<'a> AsExported<'a> {
    fn new(events: &'a [Event]) -> Self {
        let content = fold::in_order(events)
            .into_iter()
            .filter(|e| !matches!(e.kind, EventKind::Export { .. }))
            .collect();
        AsExported {
            content,
            folds: BTreeMap::new(),
        }
    }

    /// Task `id`'s row as of `at_ms`, or `None` if it didn't exist yet.
    fn row(&mut self, at_ms: i64, id: &str, fields: &[Field]) -> Option<Vec<String>> {
        let n = self.content.partition_point(|e| e.ts <= at_ms);
        let content = &self.content;
        let proj = self
            .folds
            .entry(n)
            .or_insert_with(|| fold::fold(content[..n].iter().copied()));
        let t = proj.tasks.get(id)?;
        Some(render::notion_row(t, &proj.tasks, fields))
    }
}

/// Recording an export failed after its file was written: delete the CSV and
/// the media files this command copied, so nobody imports tasks the next
/// export would send again. A file that was already beside the CSV is not in
/// `copied`, so it stays (ADR-007 amendment 4, ADR-008).
fn unrecorded(path: &Path, copied: &[PathBuf], e: Error) -> Error {
    let mut notes = Vec::new();
    match fs::remove_file(path) {
        Ok(()) => notes.push(format!(
            "{} was deleted, since the export wasn't recorded",
            path.display()
        )),
        Err(rm) if rm.kind() == ErrorKind::NotFound => {}
        Err(rm) => notes.push(format!(
            "{} was written but not recorded, and couldn't be deleted ({rm}) — delete it before importing, or its tasks will be uploaded twice",
            path.display()
        )),
    }
    notes.extend(media::remove_copied(copied));
    let note = notes.join("; ");
    let with = |msg: String| {
        if note.is_empty() {
            msg
        } else {
            format!("{msg}; {note}")
        }
    };
    match e {
        Error::Io { context, source } => Error::io(with(context), source),
        Error::Usage(m) => Error::Usage(with(m)),
        Error::Conflict(m) => Error::Conflict(with(m)),
        Error::Stale(m) => Error::Stale(with(m)),
        Error::NotFound(id) => Error::NotFound(id),
    }
}

/// Write a new file — never over an existing one (ADR-007): an earlier export
/// that hasn't been imported yet would take its tasks with it.
fn write_new_file(path: &Path, text: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                Error::Usage(format!(
                    "{} already exists — export to a new file (and if that one hasn't been imported yet, import it first)",
                    path.display()
                ))
            } else {
                Error::io(format!("couldn't create {}", path.display()), e)
            }
        })?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| Error::io(format!("couldn't write {}", path.display()), e))
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

/// Resolve `--duplicate-of`: the original must be some other task.
fn original_of(tasks: &BTreeMap<String, Task>, task: &Task, input: &str) -> Result<String> {
    let original = resolve(tasks, input)?;
    if original.id == task.id {
        return Err(Error::Usage(format!(
            "{} can't be a duplicate of itself",
            task.handle()
        )));
    }
    Ok(original.id.clone())
}

/// Etiquette: while another worker holds an active lease, the task's *state*
/// is theirs — only they may change it or close it. Metadata (title, priority,
/// labels, notes…) stays collaborative. `force` overrides (e.g. a human
/// cleaning up after a crashed agent).
fn guard_holder(task: &Task, ctx: &Ctx, force: bool, action: &str) -> Result<()> {
    match &task.lease {
        Some(l) if !force && l.is_active(ctx.now_ms) && !l.is_held_by(&ctx.actor, &ctx.node) => {
            Err(Error::Conflict(format!(
                "{} is held by {} ({}) — only the holder can {action}; wait for it to finish, reclaim it (hippo-task reclaim {}), or pass --force",
                task.handle(),
                who(&l.holder, &l.node),
                hold_status(l, ctx.now_ms),
                task.num
            )))
        }
        _ => Ok(()),
    }
}

/// After a claim attempt: do *we* (this actor on this node) hold it?
fn holds(task: &Task, ctx: &Ctx) -> Result<()> {
    match &task.lease {
        Some(l) if l.is_held_by(&ctx.actor, &ctx.node) => Ok(()),
        Some(l) => Err(Error::Conflict(format!(
            "{} is held by {} ({}) — back off and pick another task",
            task.handle(),
            who(&l.holder, &l.node),
            hold_status(l, ctx.now_ms)
        ))),
        None if task.state.is_closed() => Err(Error::Conflict(format!(
            "{} is {} — it can't be claimed; reopen it first: hippo-task update {} --state todo",
            task.handle(),
            task.state,
            task.num
        ))),
        None => Err(Error::Conflict(format!(
            "{}: the claim was not granted",
            task.handle()
        ))),
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

/// Other commands only check that a linked file is there (ADR-008).
fn present(store: &Store, task: Task) -> Task {
    media::warn_about(store, std::slice::from_ref(&task), false);
    task
}

fn present_all(store: &Store, tasks: Vec<Task>) -> Vec<Task> {
    media::warn_about(store, &tasks, false);
    tasks
}

/// Pull one task out of a projection (by exact id).
fn task_in(mut proj: Projection, id: &str) -> Result<Task> {
    proj.tasks
        .remove(id)
        .ok_or_else(|| Error::NotFound(id.to_string()))
}

/// The current state of each of `tasks`, in the same order.
fn tasks_in(mut proj: Projection, tasks: &[Task]) -> Result<Vec<Task>> {
    tasks
        .iter()
        .map(|t| {
            proj.tasks
                .remove(&t.id)
                .ok_or_else(|| Error::NotFound(t.id.clone()))
        })
        .collect()
}
