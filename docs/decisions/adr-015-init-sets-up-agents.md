# ADR-015 — `init` sets up the project's agents

**Status:** Accepted · **Date:** 2026-10-20

## Context
After installing the binary, getting a project's agents working took four steps, two of them hand edits:

1. `hippo-task init`
2. `hippo-task skill --to .claude/skills`
3. Open AGENTS.md (or CLAUDE.md) and add a line that points agents at hippo-task.
4. Merge a JSON block of SessionStart/SessionEnd hooks into `.claude/settings.json`, next to whatever is already there.

`init` printed all four, and people said the hand edits are the worst part of setting up. ADR-014 chose "hippo-task never edits an agent's settings itself". That kept setup explicit, but it left the merge to the person, in a file with other hooks and permissions in it.

Codex had no setup at all. Real use since 2026-10-08 is all Codex. No Codex skill is installed anywhere, Codex agents pick their node names by hand, and nothing gives their claims back when a session ends. Checked on 2026-10-20 against Codex's docs and codex-cli 0.163:

- **Skills:** Codex reads `.agents/skills/` at the repository root (and `~/.agents/skills/`), in the Agent Skills format `skill` already writes.
- **Hooks:** `SessionStart` and `SessionEnd` exist, in `.codex/hooks.json`, with the same shape as Claude Code's (`{"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": …}]}]}}`). Their stdin JSON carries `session_id` and `cwd`. Codex runs a project's hooks only after the person trusts them in `/hooks`, and runs them again after any change only once they're trusted again.
- **No env file:** a Codex hook can't set variables for later shell commands; nothing works like `$CLAUDE_ENV_FILE`. A `SessionStart` hook's plain stdout is added to the agent's context. Every command runs in a fresh shell, so an agent has to put its identity in front of each command, which Codex agents already do.
- **`SessionEnd`** runs when a conversation closes normally, is archived, or sits idle for 30 minutes. Its timeout is 1 second by default, 3 at most.
- **Session ids are UUIDv7.** The first 8 characters are a clock reading and repeat for sessions started within about 65 seconds of each other (`01a1363f-0a73-7180-…`). The last 12 are random.

## Decision
1. **`init` sets up agents.** At a terminal, plain `hippo-task init` asks one more question: which agents work here (Claude Code and Codex, the default; Claude Code; Codex; or neither). Scripts pass `--agents claude,codex`. As before, `--here` or `--folder` means no questions, so without `--agents` they set up no agents.
2. **For each agent, in the project root:**

   | | skill | hooks |
   |---|---|---|
   | Claude Code | `.claude/skills/hippo-task/` | `.claude/settings.json`: `hippo-task hook session-start` / `session-end` |
   | Codex | `.agents/skills/hippo-task/` | `.codex/hooks.json`: `hippo-task hook session-start --agent codex` / `session-end --agent codex`, the end hook with `"timeout": 3` |

   The skill is rewritten each time, as `skill --to` does. Hooks are merged. A hook is added only when that event has no command already running it, so `init` can be re-run safely. Every other key, hook and value is kept, in its order and with its formatting. If the file isn't a JSON object, or its `hooks` isn't one, `init` leaves the file alone and warns, showing what to add by hand. The store is still set up.
3. **One line of instructions.** "Tasks are coordinated with hippo-task: use its skill, or run `hippo-task guide` if you have none." `init` appends it to AGENTS.md. If only CLAUDE.md exists, the line goes there. If neither exists, `init` creates AGENTS.md, or CLAUDE.md when Claude Code is the only agent. A file that already mentions hippo-task is left alone. The line is only appended: nothing in the file is rewritten.
4. **A Codex session's identity comes from its start hook.** `hook session-start --agent codex` prints one plain line to stdout, even with `--json`, since Codex reads it as context. The line names the session's identity (`agent:codex` on `codex-` plus the **last** 8 letters or digits of its session id) and tells the agent to put `HIPPO_ACTOR=… HIPPO_NODE=…` in front of each `hippo-task` command. It prints nothing when the person launched Codex with both variables set, or when the project has no store. `hook session-end --agent codex` gives back what that node holds. Claude Code nodes keep the first 8 characters: Claude Code's session ids are random throughout, and existing sessions keep their node.
5. **Running `init` again is how a project gets the new skill after an upgrade.** On a project that already has a store, `init` needs no `--here` (it never moves a store) and, at a terminal, asks only about agents.
6. This supersedes ADR-014's "hippo-task never edits an agent's settings itself" and its decision 4 (the hand-added AGENTS.md line). The consent is the question or the flag. `init` reports every file it wrote, in its text and in `--json`.

## Alternatives
- **A separate `hippo-task agents` command.** Rejected: one more command to learn, while `init` is already the setup step people run, and safe to re-run.
- **Derive a Codex identity from `CODEX_THREAD_ID`.** Codex puts it in its shells' environment, but its documented environment variables don't list it. Identity would also become implicit, the reason ADR-014 rejected `CLAUDE_CODE_SESSION_ID`. Deferred: revisit if Codex agents don't follow the start line.
- **Codex's `shell_environment_policy.set`.** It can only set the same values for every session, so every session would share one node.
- **Hooks in `.claude/settings.local.json`.** That file is personal. The skill is committed for the whole team, and the hooks go with it, as README.md already recommended.
- **Parse settings files into `serde_json::Value` and write them back.** Rejected: it sorts every object's keys, so `init` would reorder a person's settings. Turning on serde_json's `preserve_order` would avoid that, but it would also change the key order of every `json!` value this crate writes, ledger event data included. A small reader keeps the keys in file order and writes untouched values back byte for byte (serde_json's `raw_value` feature).

## Consequences
- After installing, setup is `hippo-task init`, plus trusting two hooks once in Codex's `/hooks`. `init` says so when it adds Codex hooks.
- A committed hook fails, harmlessly, for a teammate without hippo-task on their PATH. This was already true of the README's snippet.
- A Codex agent that ignores the start line and picks its own node keeps its claims after the session ends, as every Codex session did before this change. The guide tells agents to use the identity their session started with.
- Checked live on 2026-10-22 with `codex exec` (codex-cli 0.163) in a scratch repository. The start hook's input carried `session_id` and `cwd`. The agent put the identity from the start line in front of every command, `whoami` included. The end hook (`reason: "other"`) gave its claim back. In the same repository without trusted hooks, the agent followed the guide and picked a node of its own, and its claim stayed held after the session ended.
- A Codex node is drawn from a random session id, like a Claude Code node. Nothing about the person or the machine is recorded.
- The docs tests apply: README.md shows `init --agents`, and AGENTS.md documents `init --json`'s new `agents` and `instructions`.
