# Changelog

Notable changes to HippoTask (`hippo-task`). Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer (while 0.x, a minor bump may break things — it will say so here).

## [0.7.1] - 2026-10-09 — internal release

### Added

- `hippo-task whoami` (ADR-014 amendment): who commands act as —
  `agent:claude@claude-7c488009`; `--json` prints `{"actor", "node",
  "default"}`. Agents can run it under an allowlist that permits only
  `hippo-task` commands, where the guide's old check
  (`echo "$HIPPO_ACTOR@$HIPPO_NODE"`) and `printenv` are refused.

### Changed

- The agent guide (and so the skill) checks identity with `whoami --json`,
  sets one only when nothing is set, never offers a node name to copy, and
  says to keep one node for the whole session. The missing-node error drops
  its `HIPPO_NODE=claude-1` example for the same reason: two agents that copy
  one name share a node and stop blocking each other.

- The review UI shows what a saved task still misses from the project's
  format in its editor, in plain words ("This project's task format also asks
  for: Project · text under “Done when”"), instead of CLI commands in the
  warning banner. Save responses gain `format_gaps`; `warnings` are unchanged.
- README: after upgrading, re-run `hippo-task skill --to …` where the skill is
  installed, so agents read the new version's protocol.

## [0.7.0] - 2026-10-08 — internal release

### Added

- `hippo-task similar "<text>"` (ADR-013): the tasks sharing the most
  distinctive words with what you're about to file, best first — rare words
  count most, identifiers like `session_token` stay whole, any state, marked
  duplicates skipped. `--json` prints task objects, like `list`. Replayed on a
  real 228-task backlog, it ranked the original of every known duplicate first.
  `add` also warns when an open task already has the same title (ignoring case
  and punctuation), and still creates it.
- A task format (ADR-012): `[format]` in the store's `config.toml` declares a
  guide for writing tasks, a description template, required fields, and
  required sections. `hippo-task format` shows it (`--json`: `config`, `guide`,
  `template`, `required_fields`, `required_sections`). `add`, `desc`, and an
  `update` of the description or a field warn once per gap, naming the fix —
  nothing is refused. The UI starts new tasks from the template and marks
  required fields; its state gains `format`.
- `hippo-task skill --to <folder>` (ADR-014) writes the agent guide as an
  Agent Skill (`hippo-task/SKILL.md` plus `references/json.md`), generated from
  the installed binary: agents load a two-line description at startup and the
  protocol only when they work on tasks, instead of reading the guide every
  session. Any agent that reads the agentskills.io format can use it.
- `hippo-task hook session-start | session-end` (ADR-014), opt-in Claude Code
  hooks: each session gets its own identity (`agent:claude` on node
  `claude-<8 characters of its session id>`, through `$CLAUDE_ENV_FILE`), and
  gives back what it holds when it ends. An identity the person launched with
  is kept; a project without tasks is left alone. `init` suggests them.

- Bulk UI editing for status, priority, owner, tags and declared fields, with
  Shift-click selection, per-task results, and conditional undo. Unsaved drafts,
  active claims and concurrent changes stay protected; uncertain saves stop the
  batch rather than being retried automatically. Unassigning or clearing a field
  is an explicit menu choice, never a blank value. List rows show who holds a
  task.
- `make integrity` also guards the UI behavior tests (`ui/*.test.cjs`) against
  removed assertions and newly skipped tests.
- A compact list-first workspace, status groups, sorting, sidebar views,
  collapsible filters and a persistent detail panel. The list remains selectable
  while reviewing descriptions and screenshots. Keyboard shortcuts include
  search, new task, select-all in the list, save and task navigation.

- Task review navigation with a compact task list, Previous / Next, a task
  chooser, and Save & next that advances only after successful saves. Drafts
  survive task switches; existing descriptions open in formatted preview.
- A screenshots/files strip, enlarged media viewer, and clipboard screenshot
  paste using the existing authenticated local upload path. Plain text pastes
  and explicit-save behavior are preserved.
- `hippo-task ui`: explicitly launched local board/list UI, packaged in the
  binary. Search/filter, create tasks, edit rows or a shared task panel, and
  move cards between states while respecting existing claims. Saves append
  ordinary events as `human:local`; no task or event schema change.
- Conditional human edits protect against stale metadata and preserve drafts
  on failure; description-only edits retain paragraph merging.
- Description **Preview / Edit text** switch renders unsaved Markdown, tables,
  task lists, and authenticated store images/video. Preview records nothing,
  escapes raw HTML, and never loads external images.
- **Add screenshots / files** in the task editor selects local images/videos, copies
  validated files into the store, and inserts Markdown links into the draft.
  Shows upload progress and errors, reuses configured media limits and
  deduplication, and attaches files to the task only on explicit Save.
- Review and export an exact selection to a Notion-ready CSV. Read-only
  previews are checked again before saving, including blocker changes and
  export bookkeeping. Previously exported tasks require explicit inclusion.
- Loopback-only HTTP server with per-launch token, origin/host checks, and
  embedded assets. No Node, Python, service, account, telemetry, or automatic
  store initialization is required for the product UI.

### Changed

- The agent guide (`guide`, AGENTS.md section A) no longer describes the review
  UI's internal HTTP API — that moved to section B, for agents changing the UI.
  It tells agents to look for similar tasks before filing, to read the task
  format, and how to pass their identity when their shell doesn't keep
  variables between commands.
- Wider task reader, plain-language labels, and expandable organization and
  workspace details for nontechnical reviewers. `make verify` also runs
  dependency-free browser logic tests with Node 18+ (development only).
- `make ui` opens the real-project UI; `make playground-ui` retains the scratch
  development playground. Direct Notion publishing and general spreadsheet CSV
  profiles remain deferred.
- Exit code 5 (`stale`) also covers conditional UI edits and export previews.
  Existing CLI command output shapes and on-disk event shapes are preserved.

## [0.6.0] - 2026-10-06 — internal release

Descriptions are markdown, with a screenshot or a short recording beside the words ([ADR-008](docs/decisions/adr-008-descriptions-markdown.md)). No new event kind: the description is still the `body` of `create` and `set-body`, so an older binary still reads the store and shows the links as text. New options, a new JSON field, and exit code 5 are why this is 0.6.0.

### Added

- `desc`, `add --body`, and `update --body` store markdown. An image link to a local file is checked, copied into the store's `media/` folder under its SHA-256, and the link rewritten to `media/<sha256>.<ext>`. PNG, JPEG, GIF, WebP, MP4, QuickTime (`.mov`), and WebM are accepted by their leading bytes; anything else — including HEIC, AVIF, and Matroska — is refused. URLs are left as text and never fetched. Image syntax inside code is text. Only an inline link (`![caption](path)`) is copied in: a reference-style image of a local file is refused. The default caps are 8 MiB for an image and 64 MiB for a video; `[media]` in `config.toml` raises them.
- `desc --file` reads the description from a file (`--file -` reads stdin). Paths in that file are relative to the file's folder. `desc --base <seq>` merges a concurrent edit by paragraph. A paragraph only one side changed is kept; the same change on both sides is kept once; paragraphs both sides changed differently exit 5 (`stale`) and write nothing.
- The description stays collaborative: anyone may edit it, holder or not. `--base` is what prevents a silent overwrite.
- Task objects gain `media` (caption, sha256, mime, bytes, path — one entry per store link). `show` prints each file's path. `show` and `export` warn when a file is missing or its bytes don't match its name; other commands only warn when it's missing. `list --search` matches captions, prose and link URLs, not an image's target such as the `media/…` path.
- Notion export copies the files those rows link into `media/` beside the CSV, and says they stay on disk to be added by hand — Notion's importer doesn't upload them. `export --json` adds `media_files`, the files that command copied. A file already there with different bytes stops the export. If recording the export fails, the CSV is deleted; the copied files stay, since another export may link them, and they import nothing on their own.

### Changed

- Within 0.6.x, JSON fields are only added.

## [0.5.0] - 2026-10-03 — internal release

Organize with fields, upload to Notion: sort tasks by project, team, or whatever you declare, then move them to Notion ([ADR-006](docs/decisions/adr-006-fields.md), [ADR-007](docs/decisions/adr-007-notion-export.md)). Two new event kinds, so everyone sharing a store upgrades together: 0.4.x skips them with a warning.

### Added

- **Fields**: attributes with one value per task — `project`, `team`, or anything declared in the store's `config.toml`, optionally limited to a list of allowed values. Set them with `add` / `update --field name=value`, clear one with `update --clear-field`, filter with `list --field`. An undeclared field or a value off the list is refused with the closest match. `hippo-task fields` shows what's declared, how much each value is used, and values tasks still carry that the config no longer allows.
- `list --label` filters by label (repeatable). Labels stay free-form.
- **`hippo-task export notion`** writes the CSV Notion imports: Name, Status, Priority, one column per field, Tags, Assignee, Description, Blocked by, hippo-task ID. With `--out` it writes a new file — never over an existing one — and records the export on each task, so the next export skips them; without it, it's a preview that records nothing. `--field`, `--label`, `--all` and `--again` choose the tasks. Tasks whose row changed since their export — closed ones included — are named, so they can be fixed in Notion; a note alone isn't a change.
- A field's `display_name` must differ from the export's own columns and from other fields' (ignoring case). A top-level setting in `config.toml` this version doesn't know is a warning, not an error, so an older hippo-task keeps working with a newer config.
- Task objects gain `fields` and `exported`; `fields --json` and `export --json` print reports (see AGENTS.md).

### Changed

- A ledger line written by a newer hippo-task now says to upgrade, instead of looking damaged.
- `--help` points agents to `hippo-task guide`, and says exit 4 means held by another worker or starting a closed task.

## [0.4.1] - 2026-10-02 — internal release

Portable: install and run it on macOS, Linux or Windows, with no Rust needed. Nothing changes for agents — same commands, same JSON.

### Added

- Prebuilt binaries for every release, each with a SHA-256 checksum: macOS (Apple silicon and Intel), Linux (x86_64 and arm64, statically linked, so they run on any distribution) and Windows (x86_64). Pushing a version tag builds and publishes them, then installs them on each OS as a check.
- `scripts/install.sh` installs the right build on macOS or Linux with one line, checks its checksum, and upgrades in place when run again.
- CI runs on Windows too: clippy and the whole test suite.

### Fixed

- Windows: creating a store no longer warns that its folder couldn't be flushed to disk. Windows can't flush a folder; the ledger file itself is still flushed on every write.
- Windows: a typed `~/` (or `~\`) at the `init` prompt means your profile folder, and paths show as `C:\…` rather than `\\?\C:\…`.

## [0.4.0] - 2026-09-27 — internal release

Where tasks live: chosen once, found from anywhere ([ADR-005](docs/decisions/adr-005-where-tasks-live.md)). **This release breaks things:** commands no longer create a store by accident.

### Breaking

- Outside a project that has a store, commands exit 2 and ask for `hippo-task init` instead of creating `.hippotask/` in the current folder. Existing `.hippotask/` folders keep working with no setup. `--dir` / `HIPPO_DIR` still name a project explicitly and, as before, create its `.hippotask/` on the first write.

### Added

- `hippo-task init` chooses where a project's tasks live: in the project (`.hippotask/`, the default) or in another folder (`--folder`), which the project points to from `.hippotask/store.json`. At a terminal it asks; otherwise pass `--here` or `--folder`. It sets up the enclosing repository's root when run from a subfolder, and never moves an existing ledger.
- Inside a git repository, `init` keeps the tasks out of git by default with a `.gitignore` in the store folder — never touching the repository's own — and `--keep-in-git` lets them be committed. A pointer is always kept out of git, since it names a path on this machine.
- Every command finds the project's store from the current folder upward, the way git finds `.git`, without climbing out of a repository. An agent working in a subfolder no longer starts a second ledger there.
- `hippo-task guide` prints the agent protocol from the installed binary, so a project's AGENTS.md only has to point to it. `init` suggests that line, an identity per agent window, and a Claude Code session-end hook that runs `hippo-task release --all`.
- The pointer carries a `kind` (`local` today), so a hosted store can be added later; a store of a kind this version can't open is a clear error.

### Fixed

- A closed task is never `blocked`. Before, a cancelled or done task still showed as blocked while its blocker was open.

## [0.3.0] - 2026-09-27 — internal release

Duplicates: find before filing, mark when found ([ADR-004](docs/decisions/adr-004-duplicates.md)). **Upgrade every tool that shares a ledger together:** 0.2.x can't read `duplicate-of` relations and skips those lines with a warning.

### Added

- `list --search <text>`: tasks whose title or description contains the text, ignoring case, in any state. Repeat it to require several terms. AGENTS.md now asks agents to search before they `add`.
- `update <id> --duplicate-of <id>`: links a duplicate to its original and cancels it in one step, so it never counts as finished work. Closing etiquette applies; a task can't be a duplicate of itself, and `--state` can't be combined with it.
- The relation `duplicate-of` in the task JSON's `relations`. Only `blocked-by` affects `blocked`.

## [0.2.0] - 2026-09-26 — internal release

Claims replace timed leases ([ADR-003](docs/decisions/adr-003-claims-without-timers.md)). **This release breaks things — upgrade every tool that shares a ledger together:** 0.1.x skips `claim` and `reclaim` lines with a warning.

### Breaking

- `start` records a **claim, with no timer**: the task is yours until you finish or release it, or someone reclaims it. Timed leases already in a ledger keep expiring as they did.
- The `lease` command and `--minutes` are retired. `hippo-task lease` now exits 2 with a pointer to `start`; `start --minutes` is an unknown flag (exit 2).
- In the task JSON's `lease` object, `expires_ms` is `null` for a claim.

### Added

- `release --all`: give back everything this worker (actor + node) holds — for a session's exit hook or a workflow's cleanup.
- `reclaim <id>` and `reclaim --from <node>`, with `--reason` and `--force`: a person — or, with `--force`, the orchestrator that launched a worker — takes back tasks a worker can't give back. The history records who took them back, from whom, and why; a reclaim naming a worker that no longer holds the task changes nothing. `release --all` and `reclaim` print arrays of task objects.
- `list --held`, and `since_ms` / `last_seen_ms` on the `lease` object: who holds what, and when each holder was last seen (any event it writes on its task counts).
- Event kinds `claim` and `reclaim`; the existing kinds keep their frozen shape.
- MIT license (`LICENSE`, and `license = "MIT"` in `Cargo.toml`).
- `list --ready`: the tasks someone could pick up right now — open, not blocked, and held by nobody, including `doing` work whose 0.1.x lease ran out. AGENTS.md's pick step uses it ([ADR-002](docs/decisions/adr-002-released-work.md)).
- `release --json` includes `released`: true if you held the task and gave it back, false if there was nothing of yours to release. Additive — the task object is unchanged.
- `make play` — the playground's interactive REPL (`playground/play.sh`) as a Makefile verb.
- `make doctor` warns when `python3` is missing (`make ui` needs it).
- Tests: the `--json` shapes (task, lease, event) are frozen by a CLI test, the way the on-disk event already was; closed-task transitions are pinned (`update --state todo` reopens, `done` on a done task is a recorded no-op, `start` on a closed task is exit 4, a forced state change leaves the other worker's claim in place).

### Changed

- `release` by the holder returns a started (`doing`) task to `todo`, so unfinished work goes back in the queue instead of sitting in `doing` with nobody on it. The fold and the event format are unchanged, and a release by anyone else still changes nothing ([ADR-002](docs/decisions/adr-002-released-work.md)).
- Playground scripts (`demo.sh`, `play.sh`, `serve.py`) build with `--locked`, like the Makefile.
- The playground UI's Live mode reads lease activity from the CLI's `lease.active` instead of the browser clock.
- Conflict messages say who holds a task and how long they've been quiet — `held by agent:codex@cx (quiet 7m)` — instead of a lease's time left; `list` shows a claim as `held:<worker>`.

### Fixed

- A person can now free a crashed worker's task (`reclaim`). Before, `--force` only changed the task's state and left the dead worker's lease in place until it expired.

- CI: the test-integrity gate no longer passes vacuously on `workflow_dispatch` (empty base) or on a branch's first push (all-zero base); it falls back to a usable base commit.
- Docs realigned with the shipped model: README and AGENTS.md say exactly when a closed task is a conflict, and describe the lints, the integrity gate, and CI as they are.

## [0.1.0] - 2026-09-25 — internal release

The first version meant for daily use: one human plus several coding agents, on one machine.

### HippoTask's new core

- **This Rust CLI + library is now HippoTask's core**, replacing the TypeScript prototype (kept at tag `v0-typescript`; its docs are in `docs/archive/typescript-v0/`).
- Renamed from the trial CLI `tasks`: the command is `hippo-task`, the variables are `HIPPO_DIR` / `HIPPO_ACTOR` / `HIPPO_NODE`, and the ledger lives at `.hippotask/ledger.jsonl`. To keep a 0.0.1 ledger: `mkdir -p .hippotask && mv .tasks/ledger.jsonl .hippotask/`.

### Fixed — bugs found by probing 0.0.1 before release

- **Labels passed to `add` were silently lost** — 28 of 40 in a probe. The create and its labels were stamped in the same millisecond, so a label could sort before its task existed and be dropped. Timestamps are now strictly increasing within a ledger.
- **A lease taken right after a create could vanish** the same way (across processes).
- **A torn last line — a crash mid-write — swallowed the next event.** Writers now fence it off first.
- **Two windows of the same agent could both hold one lease** (the holder was only the actor). A lease now belongs to actor + node.
- **Panics:** an invalid-UTF-8 line made the whole ledger unreadable; `hippo-task list | head` crashed on the closed pipe; a huge `--minutes` overflowed. Now: a warning, a clean exit, and a usage error.
- `show 12` could show a different task (digits fell through to id-suffix matching).
- `update --block 99` created a relation to a task that doesn't exist.
- A mistyped `--dir` silently created a new, empty task list.
- Every error exited 0.
- Your OS username was written into every event by default. See *Changed*.

### Fixed — found by an independent (different-model) review before release

- **Agents that didn't set `HIPPO_NODE` all shared the node `local`**, so two windows of one agent were the same worker and the lease stopped protecting them. Agent actors must now name their node (exit 2 otherwise).
- **Any worker could change the state of a task someone else held** (e.g. reset `doing` → `todo`); only closing was guarded. Now only the holder changes a leased task's state.
- **A write whose fsync failed was reported as failed but stayed in the ledger**, so a retry could duplicate it. Failed writes are now rolled back.
- **The first write in a new folder didn't flush the new directory entry.** It now does (best effort, with a warning if the filesystem can't).
- `add --body "  "` silently dropped the description while `update --body "  "` refused it; both refuse now. Simulate mode in the playground now applies the same input rules as the CLI.

### Added

- `--json` on every command; in that mode, warnings and errors are JSON lines on stderr.
- An exit-code contract: 0 ok · 1 io · 2 usage · 3 not_found · 4 conflict.
- `update --unblock`, `update --unassign`, `update --force`, `done --force`.
- The history marks what had no effect: `(rejected)`, `(no change)`; `show --json` has `applied`.
- Serialized, durable writes: an advisory file lock (10 s timeout), fsync before success, a torn-tail fence.
- Tests: unit and property tests for the fold, plus store, ops, CLI-contract, privacy, and docs suites. `make verify` and GitHub Actions CI (`make verify` on Linux + macOS, plus the tests on the minimum Rust, 1.89); clippy deny lints that keep panics out of product code; a test-integrity gate.
- `Makefile` verb contract, `make doctor`, `AGENTS.md`, this changelog.

### Changed

- **Privacy:** the default actor is `human:local` (was `human:$USER`). Nothing about you or your machine is recorded unless you set `HIPPO_ACTOR`.
- A lease belongs to **actor + node**; renew or release it from the same node.
- **Closed tasks never hold a lease:** completing or cancelling clears it, and leasing a closed task is refused (exit 4).
- **Only the lease holder can change a leased task's state or close it** — `--force` overrides. Agents (`agent:…`) must set `HIPPO_NODE`.
- A cancelled blocker no longer blocks; only open blockers do.
- Ids: digits always mean the task number; id suffixes need 4+ characters and ignore case.
- Text output: states and priorities print lowercase (as you type them); `add` prints `added #N <id>`; workers print as `actor@node`; times are labelled UTC; `show` names blockers (`blocked-by #2 "Write docs" (todo)`).
- "Back off" and other refusals go to **stderr with exit 4** (were stdout, exit 0).
- `--dir` must already exist; read-only commands never create files.
- Titles and labels are trimmed; empty values are refused.

### Unchanged

- The on-disk event format: 0.0.1 ledgers read exactly as before (the format is now frozen by a test). Events written by 0.0.1 keep their original order — including the old same-millisecond ties.

## [0.0.1] - 2026-08-17

- Trial CLI `tasks`: single-file event ledger, fold to state, lease coordination, derived blocked.
