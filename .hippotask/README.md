# `.hippotask/` — Local MCP Task Storage

This directory is the default persistent task store for the HippoTask MCP server when run with `FileTaskStore`.

## Planned layout

```
.hippotask/
├── tasks/{task-id}.json       One file per task — git-friendly diffs
├── projects/{project-id}.json
└── index.json                 Denormalized list for fast queries
```

Everything except `README.md` is gitignored so runtime agent state stays local. Teams that want to share tasks via git can opt in by removing `.hippotask/*` from [`.gitignore`](../.gitignore).

## Status

**Empty until Milestone 3 ships** ([docs/ROADMAP.md](../docs/ROADMAP.md)). The MCP server doesn't exist yet, so nothing reads or writes here today. The directory is committed with this README so the convention is discoverable.

## Dogfood plan

Once `@hippotask/mcp-server` is implemented, this repo will run the server against itself. Roadmap items become real `HippoTask` entries under `tasks/`, and agents working on HippoTask (human or LLM) track their own progress via MCP tool calls. The strongest demo of HippoTask is HippoTask managing HippoTask.

See [AGENTS.md](../AGENTS.md) for the full storage convention and conventions agents should follow.
