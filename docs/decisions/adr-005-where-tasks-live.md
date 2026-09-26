# ADR-005 — Where tasks live: chosen once, found from anywhere

**Status:** Accepted · **Date:** 2026-09-27

## Context
Until now a project's store was simply `<current folder>/.hippotask/`, created by the first command that wrote to it. That has two confirmed hazards:
- **Git.** In a git repository, a routine `git add -A` stages the ledger — every title and note, in plain text, permanently. Agents run `git add -A` all the time.
- **Split ledgers.** From a subfolder, commands see an empty list, and `add` quietly creates a second ledger there. An agent working in `src/` would file its tasks where nobody else looks.

Nobody ever chose where a project's tasks should live; the first command decided. hippo-task is local-only today, but a hosted store may come later, so the choice should be explicit and the mechanism should have room for more than a folder.

Two setup gaps sit next to this. Each project needs the agent protocol in its AGENTS.md, and a hand-copied version drifted twice in real use. And since claims no longer expire (ADR-003), a session that ends without `release --all` keeps its tasks until a person reclaims them.

## Decision
1. **`hippo-task init` chooses where this project's tasks live.** At a terminal it asks; scripts and agents pass flags instead, and it never prompts in `--json` mode or without a terminal. The choices:
   - **This project** (the default): `<project>/.hippotask/`.
   - **Another folder**, for keeping tasks out of the repository: the tasks live there, and the project gets a small pointer, `.hippotask/store.json` — `{"version": 1, "kind": "local", "path": "…"}`. The pointer names a path on this machine, so it is always kept out of git.
   - **Hosted** is reserved: a future `"kind": "hosted"` in the same pointer. It isn't offered until it exists.

   **Git:** if the chosen folder is inside a git repository, `init` says so and asks whether to keep the tasks out of git — yes by default — by writing a `.gitignore` containing `*` inside the store folder. It never edits the repository's own `.gitignore`. Keeping tasks in git is allowed; `init` then says plainly that anyone who can read the repository can read every note.
2. **Commands find the store from anywhere in the project.** Every command looks in the current folder and its parents for the nearest `.hippotask/`, the way git finds `.git`, without climbing above the enclosing git repository's root. It follows the pointer if there is one. If there's no store, the command exits 2 and says to run `hippo-task init` — **no command creates a store implicitly any more.** `--dir` / `HIPPO_DIR` still name the project folder explicitly and keep their current behaviour, for scripts and tests.
3. **`hippo-task guide`** prints the agent protocol (AGENTS.md §A) from the installed binary. A project's AGENTS.md then needs one line — "This project tracks tasks with hippo-task; run `hippo-task guide` before you start" — and can't drift from the binary. `init` prints that line.
4. **Work goes back when a session ends:** `init` prints a ready-to-paste Claude Code session-end hook that runs `hippo-task release --all`, and the README documents it. `init` doesn't edit Claude Code's settings itself. Details: see *Session end* below.

## Alternatives
- **A global registry** (`~/.config/hippo-task/…`) mapping projects to stores. Rejected for now: hidden state outside the project. A pointer inside the project is found by the same walk-up and is exactly where a hosted store will plug in.
- **Keep implicit creation; just write a `.gitignore` on first write.** Rejected: it fixes the git hazard but not split ledgers, and it still never asks where tasks should live.
- **Edit the repository's `.gitignore`.** Rejected: that's a shared, committed file. The store's own `.gitignore` keeps every change inside what hippo-task owns.
- **Prompt on first use of any command.** Rejected: agents trigger most first uses and can't answer prompts. Choosing where tasks live is a moment for a person.

## Session end
From the Claude Code hooks documentation:
- A `SessionEnd` hook runs when a session ends normally: `/exit`, `/clear`, logout, end of input, or resuming into a new session. It does **not** run when the process is killed or crashes, so after a crash a person still reclaims the work (ADR-003).
- The hook inherits the environment Claude Code was launched with, and so do the agent's shell commands. A window started as `HIPPO_ACTOR=agent:claude HIPPO_NODE=win-1 claude` therefore claims as that worker and, at the end, releases exactly that worker's tasks.
- Session-end hooks run on a short budget (about 1.5 seconds by default); `release --all` is a single ledger write.
- Personal hooks belong in `.claude/settings.local.json`. Claude Code keeps that file out of git only when it creates the file itself, which is one more reason `init` prints the snippet instead of writing it:

```json
{ "hooks": { "SessionEnd": [ { "hooks": [ { "type": "command", "command": "hippo-task release --all" } ] } ] } }
```

After `/clear` or a resume the hook has already given the tasks back, so an agent that carries on runs `start` again. Subagents inherit their parent's node, so an orchestrator still gives each worker its own `HIPPO_NODE`. A later option, not built now: a `SessionStart` hook could give every session its own node automatically through `$CLAUDE_ENV_FILE`.

## Consequences
- **Breaking, so 0.4.0:** outside an initialised project, commands exit 2 instead of creating a ledger. Existing `.hippotask/` folders are found as before — and now from subfolders too — so projects already using hippo-task keep working without `init`.
- `init` never moves an existing ledger. Run in a project that already has a store, it reports where the tasks are and offers only the git protection.
- The only files hippo-task writes are inside `.hippotask/` and the chosen store folder. It never edits the repository's `.gitignore` or Claude Code's settings.
- Hosted gets exactly one hook point today — the pointer's `kind` — and nothing else is built for it (staging rule).
