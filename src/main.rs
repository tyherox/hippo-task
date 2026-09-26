//! `hippo-task` — the CLI. Only argument parsing, the clock, and printing live
//! here; every rule lives in the library (`src/lib.rs` → `ops`, `fold`, `store`).
//!
//! Output contract (see AGENTS.md): results on stdout, diagnostics on stderr,
//! and an exit code from `error.rs`. With `--json`, stdout is one JSON
//! document and stderr is JSON lines.

use clap::{Parser, Subcommand};
use hippo_task::error::Error;
use hippo_task::model::{Priority, State, Task};
use hippo_task::ops::{self, Changes, Ctx, Filter, NewTask, Sort};
use hippo_task::render::{self, who, DetailView, ErrorView, ReleaseView, TaskView};
use hippo_task::store::Store;
use serde::Serialize;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const AFTER_HELP: &str = "\
Task ids: a number (3), a full ULID, or at least 4 trailing characters of one.
  In a shell, don't type '#3' unquoted — '#' starts a comment. Type 3.

Exit codes: 0 ok · 1 io (ledger unreadable, unwritable, or locked)
            2 usage (invalid input) · 3 not_found (no such task)
            4 conflict (leased by another worker, or task closed)

--json: stdout is one JSON document; stderr is JSON lines ({\"warning\":…} / {\"error\":…}).
Agents: see AGENTS.md for the coordination protocol.";

#[derive(Parser)]
#[command(
    name = "hippo-task",
    version,
    about = "Shared task state for humans and coding agents — an append-only event ledger folded into tasks.",
    after_help = AFTER_HELP
)]
struct Cli {
    /// Folder holding the .hippotask/ ledger (must already exist).
    #[arg(long, env = "HIPPO_DIR", default_value = ".", global = true)]
    dir: PathBuf,
    /// Who is acting, e.g. agent:claude or human:ana. Default: human:local — no personal data is recorded unless you set this.
    #[arg(long, env = "HIPPO_ACTOR", global = true)]
    actor: Option<String>,
    /// Which window/session is writing. Give every concurrent worker its own node: a lease belongs to actor + node.
    #[arg(long, env = "HIPPO_NODE", global = true)]
    node: Option<String>,
    /// Machine-readable output: one JSON document on stdout (see `hippo-task --help`).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

// Parsed once per process, so the size difference between variants is irrelevant.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Cmd {
    /// Create a task.
    Add {
        /// What needs doing.
        title: String,
        #[arg(long, default_value = "none")]
        priority: Priority,
        /// Description.
        #[arg(long)]
        body: Option<String>,
        /// Who should own it (durable intent — not a claim; see `lease`).
        #[arg(long)]
        assignee: Option<String>,
        /// Add a label (repeatable).
        #[arg(long)]
        label: Vec<String>,
    },
    /// List tasks.
    List {
        /// Only tasks in this state.
        #[arg(long)]
        state: Option<State>,
        /// Only tasks whose active lease you (this actor on this node) hold.
        #[arg(long)]
        mine: bool,
        /// Only blocked tasks.
        #[arg(long)]
        blocked: bool,
        /// Order (default: task number).
        #[arg(long, value_enum)]
        sort: Option<Sort>,
    },
    /// Show one task with its full history.
    Show {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
    },
    /// Change fields of a task.
    Update {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
        /// New title.
        #[arg(long)]
        title: Option<String>,
        /// New state (done/cancelled clear the lease; see --force).
        #[arg(long)]
        state: Option<State>,
        /// New priority.
        #[arg(long)]
        priority: Option<Priority>,
        /// Who should own it (durable intent — not a claim).
        #[arg(long, conflicts_with = "unassign")]
        assignee: Option<String>,
        /// Clear the assignee.
        #[arg(long)]
        unassign: bool,
        /// Replace the description.
        #[arg(long)]
        body: Option<String>,
        /// Add a label (repeatable).
        #[arg(long = "label-add")]
        label_add: Vec<String>,
        /// Remove a label (repeatable).
        #[arg(long = "label-remove")]
        label_remove: Vec<String>,
        /// Mark this task blocked by another (repeatable).
        #[arg(long)]
        block: Vec<String>,
        /// Remove a blocked-by relation (repeatable).
        #[arg(long)]
        unblock: Vec<String>,
        /// With --state: change it even if another worker holds the lease.
        #[arg(long)]
        force: bool,
    },
    /// Claim a task: take (or renew) its execution lease.
    Lease {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
        /// Lease length (1–1440). Renew before it runs out.
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(i64).range(1..=1440))]
        minutes: i64,
    },
    /// Claim a task and set it to doing, in one step.
    Start {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
        /// Lease length (1–1440). Renew before it runs out.
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(i64).range(1..=1440))]
        minutes: i64,
    },
    /// Give back a lease you hold, without completing the task.
    Release {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
    },
    /// Append a note to a task's history.
    Note {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
        /// The note, e.g. "left off at the token refresh".
        text: String,
    },
    /// Set a task's description.
    Desc {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
        /// The new description (replaces the old one).
        text: String,
    },
    /// Complete a task (and release its lease).
    Done {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
        /// Complete even if another worker holds the lease.
        #[arg(long)]
        force: bool,
    },
}

/// Why `run` stopped early. A closed stdout (`hippo-task list | head -1`) is not a
/// failure — the reader simply stopped listening.
enum Failure {
    App(Error),
    BrokenPipe,
}

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        Failure::App(e)
    }
}

impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::BrokenPipe {
            Failure::BrokenPipe
        } else {
            Failure::App(Error::io("couldn't write output", e))
        }
    }
}

fn main() -> ExitCode {
    // Decide the error format before parsing, so even argument errors can be JSON.
    let json = std::env::args_os().any(|a| a == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) if json && e.use_stderr() => {
            report(&Error::Usage(first_line(&e)), true);
            return ExitCode::from(2);
        }
        // --help / --version print and exit 0; other parse errors exit 2.
        Err(e) => e.exit(),
    };
    match run(cli) {
        Ok(()) | Err(Failure::BrokenPipe) => ExitCode::SUCCESS,
        Err(Failure::App(e)) => {
            report(&e, json);
            ExitCode::from(e.exit_code())
        }
    }
}

fn run(cli: Cli) -> Result<(), Failure> {
    let json = cli.json;
    let store = if json {
        Store::new(&cli.dir).on_warning(|w| eprintln!("{}", serde_json::json!({ "warning": w })))
    } else {
        Store::new(&cli.dir)
    };
    let ctx = Ctx::new(cli.actor, cli.node, chrono::Utc::now().timestamp_millis())?;
    let now = ctx.now_ms;
    let mut out = BufWriter::new(io::stdout().lock());

    match cli.cmd {
        Cmd::Add {
            title,
            priority,
            body,
            assignee,
            label,
        } => {
            let new = NewTask {
                title,
                priority,
                body,
                assignee,
                labels: label,
            };
            let t = ops::add(&store, &ctx, new)?;
            task_out(
                &mut out,
                json,
                &t,
                now,
                format!("added {} {}", t.handle(), t.id),
            )?;
        }
        Cmd::List {
            state,
            mine,
            blocked,
            sort,
        } => {
            let filter = Filter {
                state,
                mine,
                blocked,
                sort: sort.unwrap_or_default(),
            };
            let tasks = ops::list(&store, &ctx, &filter)?;
            if json {
                let views: Vec<TaskView> = tasks.iter().map(|t| TaskView::new(t, now)).collect();
                print_json(&mut out, &views)?;
            } else {
                for t in &tasks {
                    writeln!(out, "{}", render::list_line(t, now))?;
                }
            }
        }
        Cmd::Show { id } => {
            let detail = ops::show(&store, &id)?;
            if json {
                print_json(&mut out, &DetailView::new(&detail, now))?;
            } else {
                write!(out, "{}", render::detail_text(&detail, now))?;
            }
        }
        Cmd::Update {
            id,
            title,
            state,
            priority,
            assignee,
            unassign,
            body,
            label_add,
            label_remove,
            block,
            unblock,
            force,
        } => {
            let changes = Changes {
                title,
                state,
                priority,
                assignee,
                unassign,
                body,
                label_add,
                label_remove,
                block,
                unblock,
                force,
            };
            let t = ops::update(&store, &ctx, &id, changes)?;
            task_out(&mut out, json, &t, now, format!("updated {}", t.handle()))?;
        }
        Cmd::Lease { id, minutes } => {
            let t = ops::lease(&store, &ctx, &id, minutes)?;
            let line = format!(
                "leased {} to {} — {}",
                t.handle(),
                who(&ctx.actor, &ctx.node),
                lease_left(&t, now)
            );
            task_out(&mut out, json, &t, now, line)?;
        }
        Cmd::Start { id, minutes } => {
            let t = ops::start(&store, &ctx, &id, minutes)?;
            let line = format!(
                "started {} — leased to {} ({}), state doing",
                t.handle(),
                who(&ctx.actor, &ctx.node),
                lease_left(&t, now)
            );
            task_out(&mut out, json, &t, now, line)?;
        }
        Cmd::Release { id } => {
            let r = ops::release(&store, &ctx, &id)?;
            if json {
                print_json(&mut out, &ReleaseView::new(&r, now))?;
            } else if r.released {
                writeln!(out, "released {}", r.task.handle())?;
            } else {
                writeln!(
                    out,
                    "{}: not leased by you — nothing to release",
                    r.task.handle()
                )?;
            }
        }
        Cmd::Note { id, text } => {
            let t = ops::note(&store, &ctx, &id, &text)?;
            task_out(&mut out, json, &t, now, format!("noted on {}", t.handle()))?;
        }
        Cmd::Desc { id, text } => {
            let t = ops::describe(&store, &ctx, &id, &text)?;
            task_out(&mut out, json, &t, now, format!("described {}", t.handle()))?;
        }
        Cmd::Done { id, force } => {
            let t = ops::done(&store, &ctx, &id, force)?;
            task_out(&mut out, json, &t, now, format!("done {}", t.handle()))?;
        }
    }
    out.flush()?;
    Ok(())
}

/// Print one task: its JSON view, or the given human line.
fn task_out(
    out: &mut impl Write,
    json: bool,
    t: &Task,
    now: i64,
    line: String,
) -> Result<(), Failure> {
    if json {
        print_json(out, &TaskView::new(t, now))
    } else {
        writeln!(out, "{line}")?;
        Ok(())
    }
}

fn print_json(out: &mut impl Write, value: &impl Serialize) -> Result<(), Failure> {
    // serde_json hands back the underlying io::Error (e.g. a broken pipe) intact.
    serde_json::to_writer(&mut *out, value).map_err(io::Error::from)?;
    writeln!(out)?;
    Ok(())
}

fn lease_left(t: &Task, now: i64) -> String {
    t.lease.as_ref().map_or_else(
        || "no lease".to_string(),
        |l| render::remaining(l.expires_ms, now),
    )
}

/// Errors go to stderr: `error: …` for humans, a JSON line with `--json`.
fn report(e: &Error, json: bool) {
    if json {
        match serde_json::to_string(&ErrorView::new(e)) {
            Ok(line) => eprintln!("{line}"),
            Err(encode) => eprintln!("error: {e} (and couldn't encode it as JSON: {encode})"),
        }
    } else {
        eprintln!("error: {e}");
    }
}

/// clap's message without the usage block, e.g. "unexpected argument '--x' found".
fn first_line(e: &clap::Error) -> String {
    let rendered = e.render().to_string();
    let first = rendered.lines().next().unwrap_or("invalid arguments");
    first.strip_prefix("error: ").unwrap_or(first).to_string()
}
