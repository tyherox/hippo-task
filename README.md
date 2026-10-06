# 🦛 HippoTask

> **Hippocampus for modern work** — shared, auditable task memory for humans and coding agents.

`hippo-task` keeps a project's tasks in one append-only file in your repo (`.hippotask/ledger.jsonl`), folded into a task list. Several agents — or several chat windows of the same agent — can work from it without stepping on each other: they **claim** work — a claim has no timer: it's theirs until they finish or release it, or a person or orchestrator reclaims it — and every action, including refused claims, stays in an auditable history.

It's the reference implementation of the open agent-native task schema (`docs/schema-design.md`).

**Status: 0.7.0 — development** (one machine, one human, many agents). What changed: `CHANGELOG.md`. Agents: read `AGENTS.md`.

*History:* HippoTask started as a TypeScript prototype of a universal interop schema with platform adapters — preserved at git tag `v0-typescript`, with its research (the 10-platform schema study, provider scorecards) in `docs/archive/typescript-v0/`. This Rust core narrows the first release to local, multi-agent task memory; platform adapters come back once sync is earned.

## Install

**macOS or Linux**, no Rust needed:

```bash
curl -fsSL https://raw.githubusercontent.com/tyherox/hippo-task/main/scripts/install.sh | sh
```

It downloads the latest release for your machine, checks its SHA-256, and puts `hippo-task` in `~/.local/bin` — or next to the copy already on your PATH. Run it again to upgrade; `… | HIPPO_VERSION=v0.4.1 sh` installs a specific release.

**Windows:** download `hippo-task-x86_64-pc-windows-msvc.zip` from the [latest release](https://github.com/tyherox/hippo-task/releases/latest), unzip it, and put `hippo-task.exe` in a folder on your PATH.

**From source**, anywhere with Rust ≥ 1.89: `cargo install --git https://github.com/tyherox/hippo-task --locked` — or, in a clone, `make install` (`make doctor` checks your toolchain first).

Check it worked: `hippo-task --version`. To uninstall, delete the binary; your projects' tasks stay where they are.

## Set up a project

Choose once where a project's tasks live — from anywhere inside it:

```bash
hippo-task init          # asks: in this project (.hippotask/), or another folder
hippo-task init --here   # or choose up front: in this project…
hippo-task init --folder ~/tasks/my-project   # …or in another folder, outside the repository
```

- **Git:** if the tasks would sit inside a git repository, `init` keeps them out of it by default, with a `.gitignore` inside the store folder. It never edits your repository's own `.gitignore`. Pass `--keep-in-git` to let them be committed — then anyone who can read the repository can read every title and note.
- **Another folder:** the project keeps a small pointer, `.hippotask/store.json`, which is always kept out of git (it names a path on this machine).
- **Found from anywhere:** every command looks for the nearest `.hippotask/` in the current folder and the ones above it, never climbing out of a git repository. Without one, commands exit 2 and ask for `hippo-task init` — nothing is ever created by accident.
- **Agents:** add one line to the project's AGENTS.md — *"This project tracks tasks with hippo-task; run `hippo-task guide` before you start."* `hippo-task guide` prints the protocol from the installed binary, so it can't go stale.
- **Session end:** launch each agent window with its own identity (`HIPPO_ACTOR=agent:claude HIPPO_NODE=win-1 claude`) and add this Claude Code hook to `.claude/settings.local.json`, so the window's tasks go back when its session ends:

```json
{ "hooks": { "SessionEnd": [ { "hooks": [ { "type": "command", "command": "hippo-task release --all" } ] } ] } }
```

  It runs on `/exit`, `/clear`, logout and resume — not on a crash, which still needs a person to `reclaim`.

## Use

```bash
hippo-task guide                                 # the protocol agents follow
hippo-task add "Write the RFC" --priority high --label docs --body "Scope: the v1 schema"
hippo-task list                                  # every task, by number
hippo-task list --state todo --sort priority     # also: --mine, --blocked, --sort created|updated
hippo-task list --ready --sort priority          # what can be picked up now: open, unblocked, not held
hippo-task list --held                           # who holds what, and how long each has been quiet
hippo-task list --search token_refresh           # before filing: is this already on the list? (title or description)
hippo-task show 3                                # one task + its full history
hippo-task update 3 --priority urgent --label-add api --block 2
hippo-task update 3 --unblock 2 --unassign
hippo-task update 8 --duplicate-of 5             # a duplicate: link it to the original and cancel it
hippo-task start 3                               # claim it + set doing — no timer: yours until done or released
hippo-task note 3 "left off at the token refresh"
hippo-task desc 3 "One paragraph describing the task"
hippo-task desc 3 --file note.md          # markdown; a local screenshot is copied in
hippo-task desc 3 --base 4 "updated text" # merge with the description as of seq 4
hippo-task release 3                             # hand it back unfinished: back to todo, free for the next worker
hippo-task release --all                         # on exit: give back everything this worker holds
hippo-task reclaim 3 --reason "window closed"    # a person takes back a stuck worker's task (--from <node>: all of it)
hippo-task done 3                                # complete (and release)
```

Every command accepts `--json` (see `AGENTS.md` for the shapes). `release --json` also reports `released`: whether you actually held the task. `release --all` and `reclaim` print an array of the tasks they handed back.

**Task ids:** the number (`3`, shown as `#3`), the full ULID, or 4+ trailing characters of it. In a shell, don't type `#3` unquoted — `#` starts a comment; type `3`.

## Fields: project, team, and whatever else you sort by

Labels are free-form tags. **Fields** are attributes with one value per task — the project it belongs to, the team that owns it — declared in the store's `config.toml`, next to its ledger (`.hippotask/config.toml` for most projects). A field can list its allowed values:

```toml
[fields.project]
values = ["dashboard", "billing"]

[fields.team]
values = ["engineering-whiteboards", "design"]

[fields.customer]          # no list: any text
display_name = "Client"    # the column heading in exports (default: the name, capitalized)
```

```bash
hippo-task fields                                     # what's declared, and how much each value is used
hippo-task add "Design the toolbar" --field project=dashboard --field team=design
hippo-task update 3 --field project=billing           # one value per field: this replaces it
hippo-task update 3 --clear-field team
hippo-task list --field project=dashboard --label bug # filters combine
```

A field that isn't declared, or a value that isn't on its list, is refused with the closest match ("did you mean `dashboard`?"). Changing the config never changes a task: a value the config no longer allows stays on its tasks, and `hippo-task fields` lists it so you can fix it.

## Upload to Notion

Draft and organize tasks here, then move them to Notion — no API key or paid plan:

```bash
hippo-task export notion                              # preview the CSV
hippo-task export notion --field project=dashboard --out dashboard.csv
```

In Notion, **Import → CSV** turns the file into a new database; to add rows to an existing one, use its **••• → Merge with CSV** (the headers must match its property names). Columns: Name, Status (`Not started`, `In progress`, `Done`), Priority, one per field, Tags (the labels), Assignee, Description, Blocked by, and hippo-task ID. If an import leaves a column as plain text, switch it — Status to a Status property, Priority and the fields to Select, Tags to Multi-select — and Notion converts the values.

`--out` remembers what it exported, because Notion's import only ever adds rows: each task records an `export` event, and the next export skips it. A preview records nothing. The report names tasks that changed after their export — update those in Notion by hand, or `--again` adds them as new rows. `--all` includes closed tasks, and `--out` never overwrites a file.

## Identity: actor + node

- `HIPPO_ACTOR` / `--actor` — *who*: `agent:claude`, `human:ana`. Default: `human:local`.
- `HIPPO_NODE` / `--node` — *which window or session*. Default: `local`.
- **A claim belongs to actor + node.** Give every concurrent worker its own node, and two windows of the same agent can't both claim one task. Agents (`agent:…` actors) *must* set a node — without one, every command exits 2 and says how:

```bash
HIPPO_ACTOR=agent:claude HIPPO_NODE=win-1 hippo-task start 3    # → started #3
HIPPO_ACTOR=agent:claude HIPPO_NODE=win-2 hippo-task start 3    # → exit 4: held by agent:claude@win-1 — back off
```

## Exit codes

| exit | kind | meaning |
|---|---|---|
| 0 | — | success |
| 1 | `io` | the ledger couldn't be read, written, or locked |
| 2 | `usage` | invalid input: bad value, ambiguous or too-short id, nothing to do, `--dir` doesn't exist |
| 3 | `not_found` | no task matches the id |
| 4 | `conflict` | held by another worker — or, for `start`, the task is closed; or an agent reclaiming without `--force` |
| 5 | `stale` | the description conflicts with `--base`, or a conditional UI edit/export needs review |

Errors go to stderr as `error: …` (a JSON line with `--json`); results go to stdout.

## The rules it enforces

- **Claims:** granted if the task is open and nobody else holds it — otherwise exit 4, and the refused attempt is kept in the history as `(rejected)`. A claim has no timer: it holds until its worker finishes or releases it, or someone reclaims it. (Timed leases in ledgers written by 0.1.x still expire as they did.)
- **Whoever knows hands work back:** `release` by the holder returns a started task to `todo`, and `release --all` gives back everything a worker holds — run it from a session's exit hook or a workflow's cleanup. A worker that can't (it crashed, or its window closed) keeps its claim until a person — or the orchestrator that launched it — runs `reclaim` (agents need `--force`); `list --held` shows who's been quiet. Every reclaim is recorded, with its reason.
- **State belongs to the holder:** while someone holds a task, only they can change its state — including completing or cancelling it. Everyone can still edit title, priority, labels, notes. `--force` overrides the state for a human; it doesn't take the claim — `reclaim` does. Closing clears the claim. A closed task can't be started (exit 4) — reopen it with `hippo-task update 3 --state todo`; notes, labels, title, and priority stay editable, and `hippo-task done 3` on a task that's already done is recorded but changes nothing (on a cancelled task it completes it — no refusal).
- **Blocked is derived:** an open task is blocked while any task it's blocked by is still open. A closed task is never blocked.
- **Duplicates are found, then marked:** `list --search` matches the title, the description, image captions, and the URLs of links (a PR, an issue), ignoring case, in any state — not an image's target, such as the `media/…` path of a file. Check it before filing. `update --duplicate-of` links a duplicate to its original and cancels it, so it never counts as finished work; closing it follows the same holder rule as any other close. A duplicate link never blocks.
- **Descriptions are markdown, and anyone may edit them.** `desc`, `add --body`, and `update --body` store the source and copy local images and video into the store's `media/` folder, named by the bytes stored. Write image links inline — `![caption](path)`; a reference-style image of a local file is refused. `desc --base <seq>` merges a concurrent edit by paragraph; if both sides rewrote the same paragraph, the command exits 5 and writes nothing. The description is not reserved for the holder.
- **Merging, not clobbering:** concurrent label/relation changes all survive; for single fields the last write wins. Repeating a change (adding a label twice) is recorded but changes nothing — shown as `(no change)`.

## Data, durability, privacy

- The ledger is `ledger.jsonl` in the project's store — `.hippotask/`, or the folder chosen with `hippo-task init --folder`. Commands find it from the current folder upward; `--dir` / `HIPPO_DIR` names the project folder explicitly instead (it must exist). One JSON event per line, append-only.
- Writes are serialized by a file lock and flushed to disk (fsync) before a command reports success. A write that fails is rolled back, so a failed command never leaves half a change behind. Timestamps strictly increase, so the file's order is the true order of events — even within one millisecond.
- A crash mid-write can leave one unreadable line: every command then warns about it (never silently), and it can't damage later writes. A line written by a newer hippo-task is skipped with a warning to upgrade.
- The store's settings — the fields it declares — are in `config.toml` next to the ledger. Only the CLI reads it; editing it never rewrites history.
- **Privacy:** by default nothing about you or your machine is recorded — no username, no hostname. What you type — titles, notes, descriptions, and any actor name you choose — is stored **in cleartext, and forever** (append-only means it can't be edited out). Don't put secrets or personal data in tasks. Files linked from a description are stored byte for byte, including any metadata the device wrote — a phone photo can carry the place it was taken. Screenshots and screen recordings typically don't. Stripping that metadata is later work. If you commit the store to git, everyone who can read the repo can read it, files included. The store's own `.gitignore` ignores the whole folder (so `media/` with it). A team that commits the store anyway commits those files too, and a 64 MiB video is past the 50 MiB size at which GitHub warns.

## Optional board and list UI

Run `hippo-task ui` in a project with an existing task store. It opens a local
browser interface; the terminal process runs until you press Ctrl-C. Nothing
starts during normal CLI use, and no extra runtime or account is needed.

```bash
hippo-task ui                     # open this project's board
hippo-task ui --no-open           # print the URL for manual opening
hippo-task ui --port 8787         # choose a loopback port (default: available port)
hippo-task --dir /path/to/project ui
```

- Switch **Board / List**, search and filter, and select tasks. Selection and
  unsaved drafts survive view switches. Cancelled tasks have an explicit toggle.
- Edit a list row or open the shared task panel. **Save** writes to HippoTask;
  **Discard** abandons the unsaved draft. Enter saves a single-line row edit,
  Escape cancels it, and Ctrl/Cmd+Enter saves the open task panel. Drafts live
  only in the tab: save them before closing or restarting the UI.
- An open task has a compact review list, **Previous / Next**, a task chooser,
  and **Save & next**. Navigation follows the current filters and retains each
  unsaved draft; Save & next advances only after a successful save. Alt+Left /
  Alt+Right navigate when you're not typing, and Escape returns to the task
  list. Related tasks are clickable from **Related tasks & history**.
- Move cards between states with drag and drop or the status menu. Moving to
  Doing does not claim the task. Another worker's active claim prevents state
  changes; details remain editable.
- Concurrent edits cannot silently overwrite a stale metadata draft. Compare
  the latest values and explicitly keep your changes on that version before
  saving again. Description-only edits retain paragraph merging.
- **Export selected** previews exactly the selected tasks as a Notion-ready
  CSV. Save it to a new path, then import it in Notion. If rows or eligibility
  change after preview, refresh and review again. Previously exported tasks
  are skipped unless explicitly included again; Notion imports add rows and
  do not update existing ones. Linked media is copied beside the CSV for
  manual attachment. This is an export, not synchronization or direct publishing.

The server listens only on `127.0.0.1` and requires its per-launch session token
for task data and edits. Opening it never initializes a store. UI actions use
`human:local` and a fresh session node, ignoring the launching agent's actor/node
settings. No username, hostname, task draft, or browser preference is collected.
Descriptions have **Preview / Edit text** controls and open formatted when
they contain text. Preview renders your unsaved
Markdown, including headings, lists, tables, checkboxes, code, and stored images
or video. Choose Edit text to continue editing; only **Save changes** updates the
task. Raw HTML is displayed as text, and remote images are never fetched.

Use **Add screenshots / files** or paste a copied screenshot while the task
editor is focused. **Screenshots & files** shows the description's local media
as thumbnails; click to enlarge, and close the viewer with its button or Escape.
Links are inserted at the cursor in Edit text, or appended in Preview. Plain
text pastes remain text. PNG, JPEG, GIF,
WebP, MP4, MOV, and WebM use the existing configured limits (8 MiB per image and
64 MiB per video by default). The UI shows upload progress and errors; Save
becomes available when the upload finishes. Files are copied to the local store
when added, with generic Image/Video captions you can edit. Original filenames
are not stored. Only **Save** attaches the description to a task; discarding a
draft leaves its uploaded files in the store, as with CLI media ingestion.

## Develop

```bash
make verify     # fmt --check · clippy -D warnings · tests · test-integrity  (what CI runs)
make test       # also: make lint · make fmt · make doctor · make typecheck
make test-ui    # browser behavior tests (Node.js 18+; no packages to install)
make demo       # narrated demo that drives the real binary
make play       # terminal playground: a REPL over the real binary, with identity switching to try contention
make ui         # optional board/list UI for this project's configured task store
make playground-ui # scratch playground: real CLI or in-browser simulation
```

Read the code in this order: `src/model.rs` → `src/fold.rs` (the heart) → `src/store.rs` → `src/ops.rs` → `src/error.rs` → `src/render.rs` → `src/setup.rs` → `src/config.rs` → `src/main.rs`. The tests are the spec: `src/fold.rs` (merge rules + a property test), `tests/store.rs`, `tests/ops.rs`, `tests/cli.rs`, `tests/setup.rs`, `tests/fields.rs`, `tests/export.rs`, `tests/install.rs`, `tests/docs.rs`.

**Releasing:** bump the version and add its CHANGELOG section, land both on `main`, then push that one tag (`git push origin v0.4.1`). `.github/workflows/release.yml` builds the macOS, Linux and Windows binaries, publishes them, and installs them on each OS as a check. Running the workflow by hand is a dry run that publishes nothing.

## Scope

- **In 0.7.0:** an optional local board/list UI for reviewing, editing, and exporting selected tasks; descriptions are markdown, with images and video stored beside the ledger and merged by paragraph when two edits race; fields declared in `config.toml`, and export to Notion (CSV, with those files copied beside it) that remembers what it exported; prebuilt binaries for macOS, Linux and Windows; single-file ledger in a store chosen with `init` and found from anywhere in the project; fold to state; actor + node identity; claims without timers, handed back by release or reclaim; search, and duplicates linked and cancelled; derived blocked; JSON + exit-code contract; locking, fsync, crash tolerance.
- **Deferred until real use earns them (staging rule):** full Hybrid Logical Clock, per-task hash-chaining, snapshots/compaction, storage adapters, multi-machine sync, hosted collaboration, and direct publishing. The schema leaves room for each without a breaking change.

## Troubleshooting

- **`warning: …ledger.jsonl:N: skipped an unreadable line`** — line N is damaged (usually a crash mid-write). Everything else still works. To silence it, delete that one line by hand.
- **`error: couldn't lock …`** — another `hippo-task` process held the ledger for over 10 s (a hung or suspended process?). Find and stop it, then retry.
- **`error: no task store here or in any folder above it`** — this project hasn't chosen where its tasks live. A person runs `hippo-task init` (from anywhere inside the project).
- **`error: no such directory`** — `--dir` / `HIPPO_DIR` must point at an existing folder; `hippo-task` won't create one for you (a typo would silently start a new, empty list).
- **Something else?** [Open an issue](https://github.com/tyherox/hippo-task/issues) with `hippo-task --version`, your OS, the command you ran, and what it printed (with `--json`, an error is one line on stderr). Leave out task titles and notes — they're your project's data.

## License

MIT — see [LICENSE](LICENSE).
