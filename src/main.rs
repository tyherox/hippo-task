//! `hippo-task` — the CLI. Only argument parsing, the clock, and printing live
//! here; every rule lives in the library (`src/lib.rs` → `ops`, `fold`, `store`).
//!
//! Output contract (see AGENTS.md): results on stdout, diagnostics on stderr,
//! and an exit code from `error.rs`. With `--json`, stdout is one JSON
//! document and stderr is JSON lines.

use clap::{Parser, Subcommand};
use hippo_task::error::Error;
use hippo_task::model::{Priority, State, Task};
use hippo_task::ops::{self, Changes, Ctx, Filter, NewTask, ReclaimTarget, Sort};
use hippo_task::render::{self, who, DetailView, ErrorView, ReleaseView, TaskView};
use hippo_task::setup::{self, Choice, Place, Setup};
use hippo_task::store::Store;
use serde::Serialize;
use std::io::{self, BufWriter, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// The Claude Code hook `init` suggests, so a window's tasks go back when its
/// session ends (ADR-005).
const SESSION_END_HOOK: &str = r#"{ "hooks": { "SessionEnd": [ { "hooks": [ { "type": "command", "command": "hippo-task release --all" } ] } ] } }"#;
use std::process::ExitCode;

const AFTER_HELP: &str = "\
Task ids: a number (3), a full ULID, or at least 4 trailing characters of one.
  In a shell, don't type '#3' unquoted — '#' starts a comment. Type 3.

Exit codes: 0 ok · 1 io (ledger unreadable, unwritable, or locked)
            2 usage (invalid input) · 3 not_found (no such task)
            4 conflict (held by another worker, or task closed)

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
    /// The project folder (must already exist). Without it, hippo-task finds the
    /// project's store from the current folder upward.
    #[arg(long, env = "HIPPO_DIR", global = true)]
    dir: Option<PathBuf>,
    /// Who is acting, e.g. agent:claude or human:ana. Default: human:local — no personal data is recorded unless you set this.
    #[arg(long, env = "HIPPO_ACTOR", global = true)]
    actor: Option<String>,
    /// Which window/session is writing. Give every concurrent worker its own node: a claim belongs to actor + node.
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
    /// Choose where this project's tasks live. Asks, unless you pass --here or --folder.
    Init {
        /// Keep the tasks in this project, in .hippotask/.
        #[arg(long, conflicts_with = "folder")]
        here: bool,
        /// Keep the tasks in another folder; the project gets a pointer to it.
        #[arg(long, value_name = "FOLDER")]
        folder: Option<PathBuf>,
        /// Inside a git repository, let the tasks be committed (they're kept out by default).
        #[arg(long)]
        keep_in_git: bool,
    },
    /// Print the protocol agents follow to coordinate through hippo-task.
    Guide,
    /// Create a task.
    Add {
        /// What needs doing.
        title: String,
        #[arg(long, default_value = "none")]
        priority: Priority,
        /// Description.
        #[arg(long)]
        body: Option<String>,
        /// Who should own it (durable intent — not a claim; see `start`).
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
        /// Only tasks you (this actor on this node) hold.
        #[arg(long)]
        mine: bool,
        /// Only blocked tasks.
        #[arg(long)]
        blocked: bool,
        /// Only tasks ready to pick up: open, not blocked, and not held by anyone
        /// (includes started tasks whose 0.1.x lease ran out).
        #[arg(long)]
        ready: bool,
        /// Only tasks someone holds, and how long each holder has been quiet.
        #[arg(long)]
        held: bool,
        /// Only tasks whose title or description contains this text, ignoring
        /// case (repeat to require several). Search before you add a task.
        #[arg(long, value_name = "TEXT")]
        search: Vec<String>,
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
        /// New state (done/cancelled clear the claim; see --force).
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
        /// Mark this task a duplicate of another: link it to the original and cancel it.
        #[arg(long, value_name = "ID", conflicts_with = "state")]
        duplicate_of: Option<String>,
        /// With --state or --duplicate-of: close it even if another worker holds the task.
        #[arg(long)]
        force: bool,
    },
    /// Retired in 0.2.0: claims have no timer to take or renew (ADR-003).
    /// Kept, hidden, so an old script gets a pointer instead of a puzzle.
    #[command(hide = true)]
    Lease {
        #[arg(hide = true)]
        id: Option<String>,
        #[arg(long, hide = true)]
        minutes: Option<String>,
    },
    /// Claim a task and set it to doing. The claim has no timer: it's yours
    /// until you finish or release it, or someone reclaims it.
    Start {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
    },
    /// Give back a task you hold, without completing it (it goes back to todo).
    Release {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        id: Option<String>,
        /// Give back everything this worker (actor + node) holds — e.g. on exit.
        #[arg(long)]
        all: bool,
    },
    /// Take back work from a worker that can't give it back itself — for a
    /// person, or the orchestrator that launched it. It goes back to todo.
    Reclaim {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        #[arg(required_unless_present = "from", conflicts_with = "from")]
        id: Option<String>,
        /// Take back everything held on this node (a worker's window or session).
        #[arg(long, value_name = "NODE")]
        from: Option<String>,
        /// Why — recorded in the task's history.
        #[arg(long)]
        reason: Option<String>,
        /// Let an agent reclaim: for the process that launched the worker only.
        #[arg(long)]
        force: bool,
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
    /// Complete a task (and release its claim).
    Done {
        /// The task: its number (3), full id, or 4+ trailing characters of the id.
        id: String,
        /// Complete even if another worker holds the task.
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
    let mut out = BufWriter::new(io::stdout().lock());

    // Setting up needs no store and no identity: an agent without a node can
    // still read the guide.
    match &cli.cmd {
        Cmd::Guide => return guide_out(&mut out, json),
        Cmd::Init {
            here,
            folder,
            keep_in_git,
        } => {
            let asked = (*here, folder.as_deref(), *keep_in_git);
            return init_out(&mut out, json, cli.dir.as_deref(), asked);
        }
        _ => {}
    }

    let folder = store_folder(cli.dir.as_deref())?;
    let store = if json {
        Store::in_folder(folder)
            .on_warning(|w| eprintln!("{}", serde_json::json!({ "warning": w })))
    } else {
        Store::in_folder(folder)
    };
    let ctx = Ctx::new(cli.actor, cli.node, chrono::Utc::now().timestamp_millis())?;
    let now = ctx.now_ms;

    match cli.cmd {
        // Handled above; listed so this match stays exhaustive.
        Cmd::Init { .. } | Cmd::Guide => {}
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
            ready,
            held,
            search,
            sort,
        } => {
            let filter = Filter {
                state,
                mine,
                blocked,
                ready,
                held,
                search,
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
            duplicate_of,
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
                duplicate_of,
            };
            let t = ops::update(&store, &ctx, &id, changes)?;
            task_out(&mut out, json, &t, now, format!("updated {}", t.handle()))?;
        }
        Cmd::Lease { .. } => {
            return Err(Failure::App(Error::Usage(
                "`lease` was retired in 0.2.0: claims have no timer to take or renew. Claim a task with `hippo-task start <id>` — it's yours until you finish or release it, or someone reclaims it (ADR-003)".into(),
            )));
        }
        Cmd::Start { id } => {
            let t = ops::start(&store, &ctx, &id)?;
            let line = format!(
                "started {} — claimed by {}, state doing",
                t.handle(),
                who(&ctx.actor, &ctx.node)
            );
            task_out(&mut out, json, &t, now, line)?;
        }
        Cmd::Release { all: true, .. } => {
            let released = ops::release_all(&store, &ctx)?;
            if json {
                let views: Vec<TaskView> = released.iter().map(|t| TaskView::new(t, now)).collect();
                print_json(&mut out, &views)?;
            } else if released.is_empty() {
                writeln!(out, "you hold nothing — nothing to release")?;
            } else {
                for t in &released {
                    writeln!(out, "released {} — back to todo", t.handle())?;
                }
            }
        }
        Cmd::Release { id, .. } => {
            // clap guarantees an id whenever --all is absent.
            let id = id.unwrap_or_default();
            let r = ops::release(&store, &ctx, &id)?;
            if json {
                print_json(&mut out, &ReleaseView::new(&r, now))?;
            } else if r.released {
                // You held it, so it was open (closed tasks hold nothing), and a
                // `doing` task was just reset: either way it's `todo` now.
                writeln!(out, "released {} — back to todo", r.task.handle())?;
            } else {
                writeln!(
                    out,
                    "{}: not held by you — nothing to release",
                    r.task.handle()
                )?;
            }
        }
        Cmd::Reclaim {
            id,
            from,
            reason,
            force,
        } => {
            // clap guarantees exactly one of the two.
            let target = match (id, from) {
                (_, Some(node)) => ReclaimTarget::Node(node),
                (id, None) => ReclaimTarget::Task(id.unwrap_or_default()),
            };
            let back = ops::reclaim(&store, &ctx, &target, reason.as_deref(), force)?;
            if json {
                let views: Vec<TaskView> =
                    back.iter().map(|r| TaskView::new(&r.task, now)).collect();
                print_json(&mut out, &views)?;
            } else if back.is_empty() {
                let what = match &target {
                    ReclaimTarget::Task(id) => format!("task {id} isn't held by anyone"),
                    ReclaimTarget::Node(node) => format!("nothing is held on {node}"),
                };
                writeln!(out, "{what} — nothing to reclaim")?;
            } else {
                for r in &back {
                    writeln!(
                        out,
                        "reclaimed {} from {} — back to todo",
                        r.task.handle(),
                        who(&r.from.holder, &r.from.node)
                    )?;
                }
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

/// Which store a command uses (ADR-005). `--dir` / `HIPPO_DIR` names the project
/// explicitly — its `.hippotask/` is followed if it points elsewhere, and
/// created by the first write if it's missing, as before. Otherwise the store
/// is found from the current folder upward, and never created by accident.
fn store_folder(dir: Option<&Path>) -> Result<PathBuf, Error> {
    match dir {
        Some(dir) => {
            let marker = dir.join(setup::DIR);
            if marker.is_dir() {
                setup::follow(&marker)
            } else {
                Ok(marker)
            }
        }
        None => setup::discover(&current_folder()?),
    }
}

fn current_folder() -> Result<PathBuf, Error> {
    std::env::current_dir().map_err(|e| Error::io("couldn't read the current folder", e))
}

fn guide_out(out: &mut impl Write, json: bool) -> Result<(), Failure> {
    let guide = setup::guide();
    if json {
        print_json(out, &serde_json::json!({ "guide": guide }))?;
    } else {
        writeln!(out, "{guide}")?;
    }
    out.flush()?;
    Ok(())
}

/// `init`: choose where the project's tasks live, set it up, and tell the
/// person what to do next. `asked` is (--here, --folder, --keep-in-git).
fn init_out(
    out: &mut impl Write,
    json: bool,
    dir: Option<&Path>,
    asked: (bool, Option<&Path>, bool),
) -> Result<(), Failure> {
    let (here, folder, keep_in_git) = asked;
    // An explicit --dir is the project; otherwise the enclosing repository's root.
    let project = match dir {
        Some(dir) => dir.to_path_buf(),
        None => setup::project_root(&current_folder()?),
    };
    let choice = match folder {
        Some(folder) => Choice {
            place: Place::Folder(
                std::path::absolute(folder)
                    .map_err(|e| Error::io(format!("couldn't resolve {}", folder.display()), e))?,
            ),
            keep_out_of_git: !keep_in_git,
        },
        None if here => Choice {
            place: Place::Here,
            keep_out_of_git: !keep_in_git,
        },
        // Only a person at a terminal is asked; scripts and agents must choose.
        None if !json && io::stdin().is_terminal() => {
            let mut choice = setup::ask(&project, &mut io::stdin().lock(), out)?;
            choice.keep_out_of_git &= !keep_in_git;
            choice
        }
        None => {
            return Err(Error::Usage(
                "choose where this project's tasks live: `hippo-task init --here` (in .hippotask/) or `hippo-task init --folder <path>` (anywhere else)".into(),
            )
            .into())
        }
    };
    let setup = setup::apply(&project, &choice)?;
    if json {
        print_json(out, &InitView::new(&setup))?;
    } else {
        init_text(out, &setup)?;
    }
    out.flush()?;
    Ok(())
}

fn init_text(out: &mut impl Write, s: &Setup) -> Result<(), Failure> {
    if s.created {
        writeln!(
            out,
            "✓ This project's tasks will live in {}",
            s.store.display()
        )?;
    } else {
        writeln!(
            out,
            "This project already keeps its tasks in {}",
            s.store.display()
        )?;
    }
    if s.pointer {
        writeln!(
            out,
            "  The project points there from {}/{} (always kept out of git: it names a path on this machine).",
            setup::DIR,
            setup::POINTER
        )?;
    }
    match (&s.repository, s.kept_out_of_git) {
        (Some(_), true) => writeln!(out, "  Kept out of git: the store has its own .gitignore.")?,
        (Some(repo), false) => writeln!(
            out,
            "  ⚠ Not kept out of git. Once committed, anyone who can read {} can read every title and note, permanently.",
            repo.display()
        )?,
        (None, _) => {}
    }
    writeln!(out)?;
    writeln!(out, "Next:")?;
    writeln!(
        out,
        "  • Tell your agents. Add this line to the project's AGENTS.md (or CLAUDE.md):"
    )?;
    writeln!(
        out,
        "      This project tracks tasks with hippo-task; run `hippo-task guide` before you start."
    )?;
    writeln!(
        out,
        "  • Launch each agent window with its own identity, for example:"
    )?;
    writeln!(
        out,
        "      HIPPO_ACTOR=agent:claude HIPPO_NODE=win-1 claude"
    )?;
    writeln!(out, "  • So a window's tasks go back when its session ends, add this to .claude/settings.local.json:")?;
    writeln!(out, "      {SESSION_END_HOOK}")?;
    Ok(())
}

/// `init --json`.
#[derive(Serialize)]
struct InitView<'a> {
    project: &'a Path,
    store: StoreView<'a>,
    pointer: bool,
    created: bool,
    git: GitView<'a>,
}

#[derive(Serialize)]
struct StoreView<'a> {
    kind: &'static str,
    path: &'a Path,
}

#[derive(Serialize)]
struct GitView<'a> {
    repository: Option<&'a Path>,
    kept_out: bool,
}

impl<'a> InitView<'a> {
    fn new(s: &'a Setup) -> Self {
        InitView {
            project: &s.project,
            store: StoreView {
                kind: "local",
                path: &s.store,
            },
            pointer: s.pointer,
            created: s.created,
            git: GitView {
                repository: s.repository.as_deref(),
                kept_out: s.kept_out_of_git,
            },
        }
    }
}

fn print_json(out: &mut impl Write, value: &impl Serialize) -> Result<(), Failure> {
    // serde_json hands back the underlying io::Error (e.g. a broken pipe) intact.
    serde_json::to_writer(&mut *out, value).map_err(io::Error::from)?;
    writeln!(out)?;
    Ok(())
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
