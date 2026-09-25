# HippoTask — Vision

> **Hippocampus for modern work.**

The hippocampus is the part of the brain that converts short-term experiences into long-term memories and enables spatial navigation. HippoTask serves the same role for work: it's the memory layer that lets tasks form, persist, and flow between the tools where real work happens.

---

## What HippoTask Is

HippoTask is an **open schema** and **minimal toolkit** for task interoperability.

It provides:

- A **universal task schema** — a neutral, vendor-free representation of a task that captures what every major platform has in common, with clean extension points for what they don't.
- **Platform adapters** — lightweight, bidirectional translators between HippoTask and specific platforms (Jira, Asana, Linear, ClickUp, Trello, GitHub, Notion, Monday.com, Todoist, Microsoft Planner).
- **An MCP server** — first-class integration with AI/LLM agents via the Model Context Protocol, so agents can create, track, and sync tasks as part of their workflows.

It is published as a set of small, composable npm packages. Use one, use all, or use none and just adopt the schema.

---

## What HippoTask Is NOT

Let's be explicit about boundaries:

| HippoTask is NOT… | Because… |
|--------------------|----------|
| A web application or UI | We produce a schema and libraries, not a product. Build your own UI on top. |
| A database or persistence layer | Bring your own storage — file, SQLite, Postgres, in-memory. We validate shapes, not store them. |
| A competitor to Jira, Asana, or ClickUp | Those are full-featured products with years of UX investment. We're the **connector** between them, not a replacement. |
| An opinionated workflow methodology | No forced Scrum, Kanban, or GTD. The schema is intentionally minimal so *you* decide the process. |
| A complete sync engine | We provide sync *primitives* and adapter interfaces. Full production sync (conflict resolution, retry, queuing) is a higher-level concern you compose from our building blocks. |

---

## Three Use Cases

### UC1: Custom Platform Builders

**"I'm building my own project management tool and I want my users to sync with Jira, Linear, and Asana without me learning three different APIs."**

You're a developer building a task management product. You adopt `@hippotask/core` as your internal data model. When a user wants to connect their Jira, you add `@hippotask/adapter-jira`. Tasks flow in and out through a single interface. You never touch the Jira REST API directly — the adapter handles field mapping, status translation, and description format conversion.

**What HippoTask gives you:**
- A well-defined schema you don't have to design from scratch
- Adapter packages that abstract away platform-specific APIs
- Bidirectional field mapping with configurable overrides
- A community maintaining adapters so you don't have to

### UC2: Platform-Agnostic Workflows

**"My team uses Linear for engineering, Asana for marketing, and Notion for product. I need one place to see everything."**

You don't want to build a product — you want a **data layer**. HippoTask normalizes tasks from all your tools into one schema. You can build a simple dashboard, a CLI report, or even a spreadsheet export. The shape is always the same regardless of where the task lives.

**What HippoTask gives you:**
- One canonical task shape across all platforms
- Pull adapters to aggregate tasks from multiple sources
- Consistent status normalization (`todo`, `in_progress`, `done`, etc.)
- External ID tracking so you always know where a task came from

### UC3: AI / LLM Agentic Environments

**"My AI agent needs to track its own work items, and eventually push completed tasks to the team's Jira board."**

Modern AI agents (coding assistants, research agents, workflow automators) need structured task management for:
- **Bookkeeping** — tracking what's been done, what's in progress, what's blocked
- **Flow control** — breaking complex objectives into subtasks, tracking dependencies
- **Integration** — pushing results into the team's actual tools when work is done

HippoTask's MCP server exposes task CRUD, search, and sync as standard MCP tools. Any MCP-compatible AI host (Claude, GPT, Gemini, open-source agents) can use it natively.

**What HippoTask gives you:**
- An MCP server with task management tools, resources, and prompts
- Lightweight in-memory or file-backed task storage for agent sessions
- Adapter connectivity to push tasks to real platforms when ready
- Schema validation so agents produce well-formed tasks

---

## Positioning vs. Prior Art

Several projects are working in adjacent spaces. Here's how HippoTask differs:

### TaskDef (tao.ai)
TaskDef is an open standard for defining tasks, focused on AI agent orchestration with built-in QA criteria. It's YAML-based and emphasizes task *definition* (what should be done) over task *interoperability* (moving tasks between systems). HippoTask focuses on the latter — we care about field mapping, adapter plumbing, and platform sync. TaskDef could be used *alongside* HippoTask for task definition authoring.

### Open Work Lang — OWL (Plane)
OWL is an ambitious schema language covering tasks, documents, projects, and events. It's a normalization layer initiated by the Plane team. HippoTask is narrower in scope (tasks and projects only) but deeper in tooling — we ship working adapters and an MCP server, not just a schema definition language. OWL's broader scope means it may take longer to stabilize; HippoTask aims to be useful fast.

### JMAP for Tasks (IETF)
An IETF Internet-Draft extending JMAP for task sync, built on the calendar/contacts ecosystem. It's standards-track but lives in the email/PIM world. HippoTask targets the project management and AI agent ecosystem — a different audience with different needs.

### task.json / Task YAML
Lightweight community formats for local task tracking. HippoTask shares the philosophy of simplicity but adds the adapter layer and MCP integration that make tasks actually portable across real platforms.

### Commercial Alternatives (Unito, Zapier, Make)
These are SaaS products that solve sync as a hosted service. They work today, require no coding, and handle the hard parts (conflict resolution, webhooks, auth flows). But they're proprietary, not embeddable, and expensive at scale. HippoTask is the open-source, developer-first alternative.

> 📖 **For a deep dive** on all competitors with full pros/cons analysis, see [COMPETITIVE_LANDSCAPE.md](./COMPETITIVE_LANDSCAPE.md).

---

## Pros & Cons — An Honest Assessment

We believe in being transparent about what HippoTask does well and where it has limitations.

### Strengths

| Strength | Why It Matters |
|----------|---------------|
| **Open source (MIT)** | No vendor lock-in. Inspect, modify, fork, self-host. Your data never flows through our servers. |
| **Schema-first design** | A single, predictable task shape across 10+ platforms. Type-safe, validatable, documented. |
| **Composable packages** | Use just the schema (`@hippotask/core`), add one adapter, or use the whole toolkit. Pay only for what you need. |
| **AI-native from day one** | MCP server is a primary deliverable, not an afterthought. Designed for agent workflows. |
| **Multi-environment** | Core schema works in Node.js, Deno, Bun, browsers, edge runtimes, serverless. |
| **Lossless data preservation** | `external_ids`, `*_raw` fields, and `metadata` mean platform data survives round-tripping. |
| **Progressive complexity** | 2 lines to validate a task. 5 lines to connect to Jira. Complexity scales with your needs. |

### Limitations

| Limitation | Honest Assessment |
|------------|-------------------|
| **Not production-ready yet** | We're in planning phase. Competitors like Unito and Zapier work today. |
| **Requires coding** | Developers only. No drag-and-drop, no UI, no no-code option. |
| **Normalization is inherently lossy** | Flattening 10 different data models into one schema means some nuance is lost. `metadata` helps but isn't a perfect solution. |
| **Sync is genuinely hard** | We provide primitives and patterns, not a turnkey sync engine. Bidirectional sync with conflict resolution is a hard distributed systems problem. |
| **No hosted service** | You run everything yourself. No managed infrastructure. (This may change in the future.) |
| **Limited scope** | Tasks and projects only. No documents, wikis, time tracking, invoicing. Intentional, but limiting for some use cases. |
| **Adapter maintenance burden** | 10 platforms with evolving APIs = ongoing work. Community contributions help but aren't guaranteed. |
| **New and unproven** | No production users yet. No track record. Adopt with the understanding that the schema may evolve before 1.0. |

### When HippoTask Is the Wrong Choice

Be honest with yourself — HippoTask isn't for everyone:

- **"I just need Jira."** → Use the Jira API directly. HippoTask adds value when you need 2+ platforms.
- **"My team needs sync without coding."** → Use Unito or Zapier. They work today with a UI.
- **"I need documents, wikis, and tasks."** → Look at OWL/Plane or build a broader solution. We're tasks-only.
- **"I need enterprise SLA and support."** → We're open source. There's no support contract. Consider a commercial solution.

---

## Design Philosophy

1. **Schema-first** — The schema is the product. Everything else is tooling around it.
2. **Minimal core, maximal extension** — The core schema captures the universal 80%. The `custom_fields` and `metadata` fields handle the other 20%.
3. **Bring your own everything** — Storage, UI, workflow, methodology. We don't prescribe.
4. **AI-native** — MCP integration isn't an afterthought — it's a primary use case from day one.
5. **Adapters are first-class** — The schema without adapters is just a type definition. The adapters are what make it useful.
6. **Lossless round-tripping where possible** — When converting Jira → HippoTask → Jira, we preserve as much data as possible via `external_ids`, `*_raw` fields, and `metadata`.

---

## Name

**HippoTask** — from *hippocampus*, the brain structure responsible for memory formation and spatial navigation. Just as the hippocampus helps the brain organize experiences into retrievable memories and navigate space, HippoTask helps modern teams organize work into a retrievable, navigable format that flows between tools.

The hippo is also a chunky, reliable, no-nonsense animal. That fits.
