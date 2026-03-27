# docs/ — Documentation Domain Guide

> Read this before modifying anything in `docs/`.

## What This Directory Contains

Planning and design documents for the HippoTask project. These are **living documents** — they should always reflect the current state of decisions, architecture, and progress.

## Document Map

| File | Purpose | When to Update |
|------|---------|----------------|
| `VISION.md` | Project identity, use cases, non-goals, pros/cons | When scope or positioning changes |
| `COMPETITIVE_LANDSCAPE.md` | Competitor analysis, differentiation | When competitors release major updates |
| `TECH_SPECS.md` | Platform API details, field mapping, tech stack | When a platform API changes or new platform added |
| `SCHEMA.md` | Core schema specification | When schema fields are added/changed |
| `ARCHITECTURE.md` | Package structure, adapter interface, MCP design | When architecture changes |
| `ENGINEERING.md` | SOLID, TDD, code quality, DX standards | When engineering practices evolve |
| `DISTRIBUTION.md` | Publishing, multi-env support, release process | When distribution model changes |
| `PROVIDER_SCORECARD.md` | Platform openness ratings (8 dimensions) | Quarterly review or when platform APIs change |
| `EXAMPLES.md` | Example/prototype specs | When examples are added or modified |
| `ROADMAP.md` | Milestones, tasks, decision log | After each milestone completion |
| `scorecard.json` | Machine-readable provider scores | When PROVIDER_SCORECARD.md is updated |

## Update Protocol

### When writing code, update docs if:
1. **New schema field** → update `SCHEMA.md` (field definition, required/optional, validation rules)
2. **New adapter** → update `TECH_SPECS.md` (platform API section), `PROVIDER_SCORECARD.md` (scores), `scorecard.json`
3. **Architecture change** → update `ARCHITECTURE.md`
4. **New engineering pattern** → update `ENGINEERING.md`
5. **Milestone completed** → update `ROADMAP.md` (check off tasks, update decision log)

### When updating docs, also update:
- The **root `AGENTS.md`** if the monorepo layout changes
- The **package `AGENTS.md`** if you discovered new gotchas
- The **`AI_README.md`** if CLI commands or safety behavior changed

## Writing Style for Docs

- **Be concrete.** Include code examples, not just descriptions.
- **Use tables** for comparisons and reference data.
- **Show, don't tell.** A code snippet is worth 100 words.
- **Keep it current.** Outdated docs are worse than no docs.
- **Include dates** on time-sensitive information (API versions, scores).

## Gotchas

### scorecard.json must match PROVIDER_SCORECARD.md
The JSON file is the machine-readable version. When you update scores in the Markdown, update the JSON too (and vice versa).

### Markdown code blocks in SCHEMA.md are illustrative
The TypeScript excerpts in SCHEMA.md are pseudocode showing the shape of the types. The actual implementation is the Zod schemas in `packages/core/src/schema/`. If there's a conflict, **the Zod schema is the source of truth**.

### ROADMAP.md has a decision log
Every significant technical decision should be recorded in the decision log at the bottom of ROADMAP.md with date and rationale.
