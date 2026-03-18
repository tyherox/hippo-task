# HippoTask — AI Agent Guide

> **Read this file to understand how to use HippoTask for task management.**
>
> HippoTask is a CLI tool for creating, tracking, and coordinating tasks. It outputs JSON by default, making it ideal for AI/LLM agents. It includes safety features (claiming, activity logging, optimistic locking) to prevent conflicts when multiple agents work concurrently.

---

## Quick Start

```bash
# Create a task
hippotask create "Fix the login bug" --priority high --labels "auth,backend" --store ./tasks.json --agent my-agent-id

# List tasks
hippotask list --store ./tasks.json

# Claim a task before working on it
hippotask claim <task-id> --store ./tasks.json --agent my-agent-id

# Mark it done when finished
hippotask done <task-id> --store ./tasks.json --agent my-agent-id
```

**Important flags for every command:**
- `--store <path>` — Path to the task store JSON file. Always specify this for predictable behavior.
- `--agent <id>` — Your agent identifier. Use a unique string (e.g., `"claude-session-abc123"`). Used for claims and activity tracking.

---

## Complete Command Reference

### `hippotask create <title>`

Create a new task. Returns the full task object as JSON.

```bash
hippotask create "Implement user authentication" \
  --status todo \
  --priority high \
  --labels "feature,auth,backend" \
  --due 2026-03-15 \
  --description "Add OAuth 2.0 with Google and GitHub providers" \
  --store ./tasks.json \
  --agent claude-42
```

**Options:**
| Flag | Default | Values |
|------|---------|--------|
| `--status` | `todo` | `backlog`, `todo`, `in_progress`, `in_review`, `done`, `cancelled` |
| `--priority` | (none) | `none`, `low`, `medium`, `high`, `urgent` |
| `--labels` | (none) | Comma-separated strings |
| `--due` | (none) | ISO 8601 date (e.g., `2026-03-15`) |
| `--start` | (none) | ISO 8601 date |
| `--description` | (none) | Free text (Markdown supported) |
| `--project` | (none) | Project ID |
| `--parent` | (none) | Parent task ID (for subtasks) |

**Output:** Full HippoTask JSON object with auto-generated `id`, `created_at`, `updated_at`, `schema_version`, and an activity log entry.

---

### `hippotask list`

List and filter tasks. Returns `{ items: [...], has_more: boolean }`.

```bash
# All tasks
hippotask list --store ./tasks.json

# Filter by status
hippotask list --status todo,in_progress --store ./tasks.json

# Filter by priority
hippotask list --priority high --store ./tasks.json

# Search by title/description
hippotask list --search "login" --store ./tasks.json

# Only unclaimed tasks (available for work)
hippotask list --unclaimed --store ./tasks.json

# Tasks claimed by a specific agent
hippotask list --claimed-by claude-42 --store ./tasks.json

# Limit results
hippotask list --limit 10 --store ./tasks.json
```

**Output:** `{ "items": [HippoTask, ...], "has_more": false }`

---

### `hippotask get <id>`

Get a single task by ID. Supports **partial ID prefix matching** — you only need the first 8 characters.

```bash
hippotask get 019d00b0 --store ./tasks.json
hippotask get 019d00b0-09fe-720f-868f-fc801f6bef72 --store ./tasks.json
```

**Output:** Full HippoTask JSON object, or error if not found / ambiguous prefix.

---

### `hippotask update <id>`

Update one or more fields on a task. Uses **optimistic locking** — if another agent modified the task since you last read it, the update is rejected and you must re-read.

```bash
hippotask update 019d00b0 \
  --title "Updated title" \
  --status in_progress \
  --priority medium \
  --labels "revised,backend" \
  --store ./tasks.json \
  --agent claude-42
```

**Options:** Same as `create` (--title, --status, --priority, --labels, --due, --start, --description, --project, --parent).

**Output:** Updated HippoTask JSON, or error `"Conflict: task has been modified since you last read it"`.

**If you get a conflict error:** Re-read the task with `get`, then retry the update.

---

### `hippotask done <id>`

Mark a task as done. Shortcut for `update --status done`. Also sets `completed_at` and auto-releases your claim.

```bash
hippotask done 019d00b0 --store ./tasks.json --agent claude-42
```

**Output:** Updated HippoTask JSON with `status: "done"`, `completed_at` set.

---

### `hippotask delete <id>`

Permanently delete a task.

```bash
hippotask delete 019d00b0 --store ./tasks.json
```

**Output:** `{ "ok": true, "message": "Deleted task...", "id": "..." }`

---

### `hippotask claim <id>`

**Claim a task before starting work on it.** This tells other agents the task is being worked on.

```bash
hippotask claim 019d00b0 --store ./tasks.json --agent claude-42 --ttl 3600
```

| Flag | Default | Description |
|------|---------|-------------|
| `--ttl` | `3600` | Claim duration in seconds. After this, the claim auto-expires. |

**Behavior:**
- ✅ Succeeds if task is unclaimed
- ✅ Succeeds if task is claimed by the same agent (re-claim / refresh)
- ✅ Succeeds if previous claim has expired (TTL elapsed)
- ❌ Fails if task is claimed by a different agent and not expired

**Output:** Updated HippoTask JSON with claim metadata, or error `"already claimed by..."`.

---

### `hippotask release <id>`

Release your claim on a task (when you're done or can't finish).

```bash
hippotask release 019d00b0 --store ./tasks.json --agent claude-42
```

**Behavior:**
- ✅ Succeeds if you own the claim
- ✅ Succeeds if the claim has expired (anyone can clean up)
- ❌ Fails if another agent owns a non-expired claim
- ⚠️ No-op if task is unclaimed

---

### `hippotask log <id>`

Show the full activity log for a task. Useful for debugging and auditing agent behavior.

```bash
hippotask log 019d00b0 --store ./tasks.json
```

**Output:**
```json
{
  "task_id": "019d00b0-09fe-720f-868f-fc801f6bef72",
  "entries": [
    { "agent_id": "claude-42", "action": "created", "timestamp": "2026-03-08T10:00:00.000Z", "detail": "Created task: Fix login bug" },
    { "agent_id": "claude-42", "action": "claimed", "timestamp": "2026-03-08T10:01:00.000Z" },
    { "agent_id": "claude-42", "action": "status_changed", "timestamp": "2026-03-08T10:30:00.000Z", "detail": "todo → in_progress" },
    { "agent_id": "claude-42", "action": "completed", "timestamp": "2026-03-08T11:00:00.000Z", "detail": "in_progress → done" },
    { "agent_id": "claude-42", "action": "released", "timestamp": "2026-03-08T11:00:00.000Z" }
  ]
}
```

---

### `hippotask init`

Create a `.hippotask/` directory in the current project folder.

```bash
hippotask init
```

---

### `hippotask info`

Show store location, task counts, and active claims.

```bash
hippotask info --store ./tasks.json
```

**Output:**
```json
{
  "store_path": "./tasks.json",
  "agent_id": "hostname",
  "task_count": 12,
  "active_claims": 3,
  "claims_by_agent": { "claude-42": 2, "gpt-agent-7": 1 },
  "status_counts": { "todo": 4, "in_progress": 5, "done": 3 },
  "schema_version": "1.0.0"
}
```

---

## Storage Model

Tasks are stored in a JSON file. The location is determined by:

| Priority | Source | Path |
|----------|--------|------|
| 1 (highest) | `--store` flag | Explicit path you provide |
| 2 | `HIPPOTASK_STORE` env var | Environment-level override |
| 3 | Local project store | `.hippotask/tasks.json` in current directory |
| 4 (lowest) | Global store | `~/.hippotask/tasks.json` |

**Recommendation for AI agents:** Always use `--store <path>` to be explicit about which store you're using. This avoids surprises when the working directory changes.

**Per-project isolation:** Each project directory can have its own `.hippotask/tasks.json`. Run `hippotask init` in a project root to set this up. Agents working in different project directories will automatically use different task stores.

---

## Safety Protocol for AI Agents

### The 4 Safety Layers

1. **Optimistic Locking** — The store rejects writes if the task was modified since you last read it. If you get a conflict error, re-read the task and retry.

2. **File Locking** — Only one process can write to the store at a time. This is automatic — you don't need to do anything.

3. **Task Claiming** — Before working on a task, `claim` it. Other agents will see it's claimed and skip it. Release when done.

4. **Activity Log** — Every action (create, claim, update, complete, release) is recorded with your agent ID and timestamp. This provides full audit trail.

### Recommended Workflow

```bash
# 1. Find available work
hippotask list --unclaimed --status todo --store ./tasks.json

# 2. Claim a task before starting
hippotask claim <task-id> --store ./tasks.json --agent $MY_AGENT_ID

# 3. Update status as you work
hippotask update <task-id> --status in_progress --store ./tasks.json --agent $MY_AGENT_ID

# 4. Mark done when complete (auto-releases claim)
hippotask done <task-id> --store ./tasks.json --agent $MY_AGENT_ID

# If you can't finish, release the claim so others can pick it up
hippotask release <task-id> --store ./tasks.json --agent $MY_AGENT_ID
```

### Handling Conflicts

If you see `"Conflict: task has been modified since you last read it"`:

1. Re-read the task: `hippotask get <id> --store ./tasks.json`
2. Check what changed (another agent may have updated it)
3. Retry your update with the fresh data

### Claim Etiquette

- **Always claim before working.** This prevents duplicate work.
- **Use a unique agent ID.** Include your session/instance ID (e.g., `"claude-session-abc123"`).
- **Set appropriate TTL.** Default is 1 hour. For short tasks, use `--ttl 600` (10 minutes). For long tasks, use `--ttl 7200` (2 hours).
- **Release if you can't finish.** Don't let claims expire silently — release them so other agents can pick up the work.

---

## Task Schema

Every task has these fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | ✅ | Unique ID (auto-generated UUIDv7) |
| `title` | string | ✅ | Task title |
| `status` | enum | ✅ | `backlog`, `todo`, `in_progress`, `in_review`, `done`, `cancelled` |
| `created_at` | string | ✅ | ISO 8601 timestamp |
| `updated_at` | string | ✅ | ISO 8601 timestamp |
| `schema_version` | string | ✅ | Always `"1.0.0"` |
| `description` | string | | Task details (Markdown) |
| `priority` | enum | | `none`, `low`, `medium`, `high`, `urgent` |
| `labels` | string[] | | Tags for categorization |
| `due_date` | string | | ISO 8601 date |
| `start_date` | string | | ISO 8601 date |
| `completed_at` | string | | Set automatically by `done` command |
| `project_id` | string | | Group tasks by project |
| `parent_id` | string | | Create subtask hierarchies |
| `assignees` | object[] | | People assigned |
| `estimate` | number | | Effort estimate (positive number) |
| `external_ids` | object | | Map of platform IDs (e.g., `{ "jira": "PROJ-123" }`) |
| `custom_fields` | object | | Typed key-value extensions |
| `metadata` | object | | Untyped overflow (claims and activity stored here) |

---

## Workflow Patterns

### Breaking Down Work

```bash
# Create a parent task
hippotask create "Implement auth system" --store ./tasks.json --agent planner-1

# Create subtasks
hippotask create "Set up OAuth credentials" --parent <parent-id> --priority high --store ./tasks.json --agent planner-1
hippotask create "Build login page" --parent <parent-id> --priority medium --store ./tasks.json --agent planner-1
hippotask create "Add session management" --parent <parent-id> --priority medium --store ./tasks.json --agent planner-1
hippotask create "Write integration tests" --parent <parent-id> --priority low --store ./tasks.json --agent planner-1
```

### Multi-Agent Coordination

```bash
# Agent 1: Creates and plans work
hippotask create "Research API options" --store /shared/tasks.json --agent planner-agent

# Agent 2: Finds available work and claims it
hippotask list --unclaimed --status todo --store /shared/tasks.json
hippotask claim <id> --store /shared/tasks.json --agent worker-agent-1

# Agent 3: Also finds work (skips claimed tasks)
hippotask list --unclaimed --status todo --store /shared/tasks.json
hippotask claim <id> --store /shared/tasks.json --agent worker-agent-2

# Check who's working on what
hippotask info --store /shared/tasks.json
```

### Status Progression

A typical task lifecycle:

```
backlog → todo → in_progress → in_review → done
                                         → cancelled (if abandoned)
```

Use `hippotask update <id> --status <status>` to move tasks through the pipeline.

---

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `"Conflict: task has been modified"` | Another agent updated the task since you read it | Re-read with `get`, then retry |
| `"already claimed by X"` | Another agent claimed this task | Use `list --unclaimed` to find available tasks, or wait for the claim to expire |
| `"Task not found"` | ID doesn't match any task | Check the ID (use `list` to find tasks). Prefix match requires 8+ chars for uniqueness |
| `"Failed to acquire lock"` | Another process is writing to the store | Wait and retry. If persistent, the lock file may be stale — it will auto-expire (30s default) |
| `"not claimed by X"` | Trying to release another agent's claim | Only the claim owner can release. Wait for TTL expiry |
