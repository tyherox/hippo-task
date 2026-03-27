# @hippotask/cli — Domain Guide

> Read this before modifying anything in `packages/cli/`.

## What This Package Does

A **CLI tool** (`hippotask`) for task management, designed for both humans and AI agents. Features:
- 11 commands for full task lifecycle
- File-based JSON store with configurable location
- **4 safety layers** for multi-agent conflict prevention
- JSON output by default (machine-readable for AI)

## Module Map

```
src/
├── bin.ts                   ← CLI entry point (#!/usr/bin/env node)
├── cli.ts                   ← Commander setup, registers all commands
├── index.ts                 ← Programmatic API exports
├── store.ts                 ← FileTaskStore — CRUD with file/optimistic locking
├── lock.ts                  ← File lock (.lock file with TTL)
├── resolve-store.ts         ← Store path resolution (flag → env → local → global)
├── safety.ts                ← Layer 3 (claiming) + Layer 4 (activity log)
├── context.ts               ← CLI context builder (store + agent + pretty)
├── output.ts                ← JSON / pretty formatters
└── commands/
    ├── create.ts            ← hippotask create <title>
    ├── list.ts              ← hippotask list [filters]
    ├── get.ts               ← hippotask get <id>
    ├── update.ts            ← hippotask update <id> [fields]
    ├── delete.ts            ← hippotask delete <id>
    ├── done.ts              ← hippotask done <id>
    ├── claim.ts             ← hippotask claim <id>
    ├── release.ts           ← hippotask release <id>
    ├── log.ts               ← hippotask log <id>
    ├── init.ts              ← hippotask init
    └── info.ts              ← hippotask info
```

## The 4 Safety Layers

Understanding these is CRITICAL before modifying any CLI command:

### Layer 1: Optimistic Locking (`store.ts`)
`store.update(id, changes, expect_updated_at)` — if `expect_updated_at` doesn't match the current task's `updated_at`, the write is rejected with a conflict error. This prevents lost writes when two agents read → modify → write the same task.

### Layer 2: File Locking (`lock.ts`)
Every `store.create/update/delete/clear` acquires a `.lock` file before reading/writing the store JSON. The lock has a TTL stored in the lock file itself (`ttl_ms`), so stale locks from crashed processes auto-expire.

### Layer 3: Task Claiming (`safety.ts`)
Tasks can be "claimed" by an agent via `claimTask()`. Claim data is stored in `metadata`:
- `metadata["hippotask.claimed_by"]` — agent ID
- `metadata["hippotask.claimed_at"]` — timestamp
- `metadata["hippotask.claim_ttl"]` — seconds (default 3600)

Claims auto-expire. Same agent can re-claim. Different agent must wait for expiry.

### Layer 4: Activity Log (`safety.ts`)
Every action (create, claim, update, complete, release) appends to `metadata["hippotask.activity"]`. Each entry has `{ agent_id, action, timestamp, detail? }`. This is an append-only audit trail.

## How to Add a New Command

1. **Write tests first** in `tests/unit/commands.test.ts`:
   - Test the happy path (valid input → correct JSON output)
   - Test error paths (not found, invalid input)
   - Tests use `execSync` to invoke the compiled CLI binary

2. **Create `src/commands/{name}.ts`** following the pattern of existing commands:
   - Export a `register{Name}(program: Command)` function
   - Get global opts via `cmd.parent?.opts()`
   - Build context with `buildContext(globalOpts)`
   - Use `ctx.store` for data access
   - Use `appendActivity()` for audit logging
   - Output via `formatTask/formatTaskList/formatSuccess/formatError`

3. **Register in `cli.ts`**: import and call `register{Name}(program)`

4. **Update this AGENTS.md** with the new command

5. **Update `AI_README.md`** in the repo root with the new command reference

## Gotchas & Quirks

### Tests require a build first
CLI command tests invoke the compiled `dist/bin.js` via `execSync`. You must run `pnpm build` before `pnpm test` for the CLI package. Turborepo handles this automatically when you run from root.

### `--store` flag MUST be used in tests
Every test creates a temp directory and passes `--store /tmp/.../tasks.json` to isolate test data. Never rely on the default store path in tests.

### Optimistic lock timing sensitivity
Some tests need a `setTimeout(5ms)` delay between `createTask()` and `update()` to ensure `updated_at` timestamps differ (JavaScript Date has millisecond precision, and ops within the same ms get the same timestamp).

### `done` auto-releases claims
When `hippotask done <id>` is called, it checks if the completing agent owns a claim and auto-releases it. This is intentional — completed tasks shouldn't hold claims.

### Activity log is in metadata, not a separate field
The activity log lives at `metadata["hippotask.activity"]`. This means it's preserved through any operation that doesn't explicitly overwrite `metadata`. When doing `store.update(id, { metadata: ... })`, you must merge with existing metadata to avoid losing the log.

### `getActivityLog()` returns a copy
`getActivityLog(task)` returns a shallow copy of the array (`[...raw]`). Modifying the returned array doesn't affect the task. Use `appendActivity()` to add entries.

### Store path resolution order
1. `--store` flag (explicit)
2. `HIPPOTASK_STORE` env var
3. `.hippotask/tasks.json` in CWD (if `.hippotask/` dir exists)
4. `~/.hippotask/tasks.json` (global fallback)

### File lock TTL is stored in the lock file
The `ttl_ms` is written into the lock JSON itself. `isLockStale()` reads it from the lock file, not from the caller's arguments. This means a lock created with `ttlMs: 10` will be considered stale after 10ms, regardless of what TTL a subsequent acquirer passes.

## Testing Commands

```bash
pnpm --filter @hippotask/cli build    # Must build before testing
pnpm --filter @hippotask/cli test     # Run all CLI tests
```

## Programmatic API

The CLI package also exports its internals for library consumers:
- `FileTaskStore` — use the file store programmatically
- `claimTask`, `releaseTask`, `isTaskClaimed`, `getClaimInfo` — safety layer functions
- `appendActivity`, `getActivityLog` — activity log functions
- `acquireLock`, `releaseLock` — file locking
- `resolveStorePath` — path resolution logic

See `src/index.ts` for the full list.
