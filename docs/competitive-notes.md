# Competitive Notes — Open Agent-Native Task Schema

Positioning against the adjacent landscape. Kept honest: where we overlap, where we differ, and where they're ahead.

## Closest adjacent — Anthropic "Productivity" plugin (Anthropic-verified)
**What it is:** gives *Claude* persistent work context — manages a **markdown task list Claude reads/writes/executes against**, a **two-tier memory** (learns people/projects/shorthand), a **visual dashboard**, and **MCP connectors** (email/calendar/chat/trackers) for auto-discovering + syncing tasks (`/start`, `/update`).

**Real overlap:** local files an AI reads/writes; persistent AI task context. If a user just wants "Claude manages my todos with memory + a dashboard," this already does it, is verified, and has GUI + memory + connectors we don't. **We do not compete on "AI to-do assistant."**

**Where we differ (= our three focuses):**
| | Productivity plugin | Ours |
|---|---|---|
| Vendor scope | *Claude-specific* (makes Claude a colleague) | *Vendor-neutral schema* — Claude + Codex + Cursor + N windows |
| Concurrency | single-writer markdown (a list Claude edits) | event-sourced + leases → deterministic multi-writer (full HLC: deferred) |
| Audit | mutable list overwritten in place (git at best) | append-only ledger → who/when/what per task (hash-chaining, time-travel, undo: deferred) |
| Interop | syncs your tasks *into Claude's world* via MCP | portable *by the format* — plain JSON lines you own (VTODO/OSLC mapping + `refs`/`ext`: design target, sync deferred) |

*Shipped vs design target (`hippo-task 0.1.0`):* the ledger is append-only with a per-ledger monotonic clock; full HLC, per-task hash-chaining, and the VTODO/OSLC `refs`/`ext` mapping are deferred until real use earns them — see the README's "Scope" and [`schema-design.md`](schema-design.md) §11.

**One-liner:** *the Productivity plugin makes Claude better at your tasks; ours makes your tasks independent of any one AI.*

**What they do that we don't (honest):** workplace memory, a visual dashboard, official distribution/verification. Memory is a real, separate capability we are not building.

## Broader landscape (brief)
- **Local-first task tools** — Taskwarrior (local, JSON export, hooks), todo.txt, Obsidian/Logseq, org-mode. Mature but **human-first and single-writer**; none are agent-native or built for concurrent multi-agent writes.
- **Agent standards** — MCP (→ Linux Foundation) and A2A standardize *orchestration/comms*, not the **work-item record**. The portable task/work-item schema is the gap.

## The gap we occupy
A **vendor-neutral, multi-agent-concurrent, auditable task substrate** you own. Not "another todo app," not "Claude's task memory" — the neutral layer *underneath* any agent.

## Honest risks (skeptic hat)
1. **Adjacent verified incumbent.** The Productivity plugin both *validates* the space and *pressures* us (polished, verified, discoverable). If Anthropic adds multi-agent/portability, our wedge narrows.
2. **Narrow audience.** Only the multi-agent/multi-model power user feels our pain. Consistent with no-tourists, but it *is* narrow.
3. **We're behind on the appliance** (no GUI, no memory). Deliberate (substrate-first), but means we're not a drop-in replacement for the plugin's polish.

**Implication:** win by being the thing they structurally are not — neutral, concurrent, auditable — for the user who actually needs it. Prove that need in real use before building further.
