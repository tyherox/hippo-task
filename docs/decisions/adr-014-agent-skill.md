# ADR-014 — Agents learn hippo-task from a skill, not a guide read each session

**Status:** Accepted · **Date:** 2026-10-08

## Context
A project tells its agents "run `hippo-task guide` before you start" (the pilot project's AGENTS.md did exactly this). Every session then reads the whole guide, about 11 KB (≈2.8K tokens), whether or not it touches a task:

| part of the guide | size | who needs it |
|---|---|---|
| identity, the loop, rules, exit codes | 5.2 KB | every agent doing task work |
| JSON shapes | 3.7 KB | an agent parsing something unusual |
| the review UI's local HTTP API | 1.8 KB | nobody coordinating work; it says so itself |

An agent that skips the read misses the rules that matter most: identity, `--json`, and giving claims back on exit. Identity is also a chore on its own. In Claude Code, `export` doesn't last from one Bash call to the next, so the guide's `export HIPPO_ACTOR=…` doesn't hold. Agents that end up unidentified all act as `human:local@local`, and two of them both win a `start`.

Agent Skills ([agentskills.io](https://agentskills.io/specification), December 2025) fit this. A skill is a folder whose `SKILL.md` has a `name` and `description`. Agents load only those two fields (≈100 tokens) at startup, read the body when a task calls for it, and read referenced files only when needed. Claude Code reads skills from `.claude/skills/` and `~/.claude/skills/`. agentskills.io lists Codex, Gemini CLI, GitHub Copilot and Cursor among the agents reading the same format, each from its own folder.

Claude Code hooks cover identity. A `SessionStart` hook receives the session's `session_id` and may append `export` lines to `$CLAUDE_ENV_FILE`, which later Bash calls in that session pick up. A `SessionEnd` hook runs on clean exits, but not reliably on Ctrl-C or a closed window.

## Decision
1. **`hippo-task skill --to <skills-folder>` writes the skill**: `<folder>/hippo-task/SKILL.md` and `<folder>/hippo-task/references/json.md`. It creates the folders and overwrites its own files, so re-running it after an upgrade is how a skill gets updated. `--json` prints `{"skill", "files"}`. Typical uses are `--to .claude/skills` (committed, so every teammate's agent has it) and `--to ~/.claude/skills` (one person, every project).
2. **The skill is generated from AGENTS.md section A**, the text `guide` already prints, so it can't drift from the binary that wrote it:
   - `SKILL.md`: frontmatter (`name: hippo-task`; a `description` saying it coordinates work through this project's task ledger, and to use it when picking up, claiming, finishing, or filing tasks; `metadata: {hippo-task-version: "<version>"}`), then the loop, rules and exit codes.
   - `references/json.md`: the JSON shapes, linked from `SKILL.md`.
3. **The UI's HTTP API leaves section A** for section B, which is for agents changing this crate. `guide` still prints all of section A, for agents without skill support.
4. **`init` suggests the skill** in its closing text, and a project's AGENTS.md needs one line: "Tasks are coordinated with hippo-task (skill in `.claude/skills/hippo-task`; without skills, run `hippo-task guide`)."
5. **Identity and cleanup for Claude Code, opt-in through hooks:**
   - **`hippo-task hook session-start`** reads the hook's JSON on stdin. If `HIPPO_ACTOR` and `HIPPO_NODE` aren't already set, it appends `export HIPPO_ACTOR=agent:claude` and `export HIPPO_NODE=claude-<first 8 characters of session_id>` to `$CLAUDE_ENV_FILE`.
   - **`hippo-task hook session-end`** releases what that session holds. It uses `HIPPO_ACTOR`/`HIPPO_NODE` when they're in its environment, otherwise the same session-derived values.
   - README.md shows the `settings.json` snippet. hippo-task never edits an agent's settings itself.

   The two slices ship separately: the skill (1–4) first, then the hooks (5).

## Alternatives
- **An MCP server (`hippo-task mcp`).** Rejected for now. It would be a second interface to keep in step with the CLI (every command, flag and error, twice), plus a running process per session. A skill solves the per-session read using the CLI agents already drive. MCP would give each session its own identity for free, but the hooks do that for Claude Code. Revisit if an agent without a shell needs hippo-task.
- **Only slim the guide.** Not enough: even a 5 KB guide is read every session, by sessions that never touch a task.
- **A hand-written SKILL.md in this repo.** Rejected: two texts that drift. The docs tests already hold AGENTS.md to the binary, and the skill inherits that.
- **Derive identity from Claude Code's environment** (`CLAUDE_CODE_SESSION_ID`, present in Bash calls today). Rejected: it isn't a documented interface, and it would make identity implicit. The same environment carries the person's account email, and hippo-task records nothing about the user unless they opt in. A hook is documented and is an explicit choice.
- **A Claude Code plugin from this repository** (skill and hooks in one install). Deferred: the plugin's skill would follow this repo's `main` while each machine's binary follows its own release, so the two could disagree. Revisit if the README steps prove fiddly for the team.

## Consequences
- An agent that never touches tasks pays ≈100 tokens instead of ≈2.8K. One doing task work reads ≈1.3K, and the JSON reference only when it needs it.
- A committed skill goes stale when the team upgrades until someone re-runs `skill --to`. The `hippo-task-version` in its frontmatter makes that visible. A staleness check is deferred.
- The node name `claude-<8 characters>` is drawn from a random session id, not from anything about the person or machine, and is recorded only when someone installs the hook. Clearing or resuming a session starts a new session id and a new node. `session-end` gives back the old node's claims first.
- A session killed without a clean exit still strands its claims, as today: `list --held` shows the quiet holder and a person reclaims it (ADR-003).
- The docs tests apply: `hippo-task skill` and `hippo-task hook` appear in README.md, and the skill's JSON keys in AGENTS.md.
