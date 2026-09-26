# ADR-001 — Implementation language: Rust

**Status:** Accepted · **Date:** 2026-07

> *Note (2026-09-26):* hash-chaining is deferred in `hippo-task 0.1.0` (staging rule — [`../schema-design.md`](../schema-design.md) §11); the ledger shipped append-only without it. The decision below stands as written.

## Context
The schema/format is the durable commitment and is **language-agnostic (JSON) — already locked**. This decision is only about the language for the CLI/library and the eventual production core. Discriminating requirements:
- Single-binary, small, fast, callable by any agent anywhere (CLI-first; caller's language irrelevant).
- **Correctness under concurrency** — an event-sourced, hash-chained ledger with a multi-writer / multi-machine future. (The crown jewel.)
- Future surfaces: a **WASM core** shared across CLI + a possible browser GUI + cross-language bindings.
- Neutral/local-first ethos: don't drag a heavy runtime into every consumer.

## Decision
**Rust** for the CLI and the durable core. The trial CLI is *also* Rust — it graduates rather than being a throwaway.

## Alternatives considered
- **Go** — pragmatic ubiquitous CLI binary, simplest, dev-tool lingua franca. Rejected: weaker correctness guarantees and no native WASM-shared-core; Rust's ledger-correctness + WASM alignment won.
- **TypeScript + Bun** — fast to write, Tier-1 MCP, browser-native, and now single-binary via `bun build --compile` (Claude Code precedent). Was the recommended *trial* language for speed. Rejected as the *final* home: fat runtime-embedding binaries (~50–90MB) undercut the tiny-neutral-substrate ethos, and weaker correctness for a ledger. Chosen instead: build once in Rust rather than TS-then-rewrite.
- **Python** — Tier-1 MCP, but worst distribution and weakest for a robust concurrent ledger. Rejected.

## Consequences
**Positive:** correctness-under-concurrency guarantees fit the hash-chained ledger; WASM core enables a future browser GUI + bindings; tiny fast binary fits "agent-callable everywhere" + neutrality.

**Negative / risks:**
- **Slower velocity** than TypeScript or Go → validating the wedge takes longer. **Mitigation:** keep the first version's scope brutally minimal (staging rule), and **judge the wedge independently of implementation speed** — "slow to build" ≠ "wedge unproven."
- **Rust MCP SDK is community-maintained** (less mature than TS/Python). Acceptable: CLI-first is the primary agent surface; MCP is a later, optional surface.

**Revisit if:** the WASM/browser future never materializes and velocity pain dominates → Go is the standing fallback.
