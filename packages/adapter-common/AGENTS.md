# @hippotask/adapter-common — Domain Guide

> Read this before modifying anything in `packages/adapter-common/`.

## What This Package Does

Provides the **adapter interface** and **shared utilities** that all platform adapters will use. It sits between `@hippotask/core` (pure schema) and concrete adapters (platform-specific code).

**Key principle:** This package contains ZERO platform-specific code. It defines the contract and provides reusable infrastructure.

## Module Map

```
src/
├── adapter.ts               ← HippoAdapter<T> interface (THE contract)
├── types.ts                 ← HippoTaskCreate, HippoTaskUpdate
├── status-map.ts            ← StatusMap type + mapStatus/mapStatusReverse
├── priority-map.ts          ← PriorityMap type + mapPriority/mapPriorityReverse
├── errors.ts                ← AdapterError, RateLimitError, NotFoundError, AuthError
├── rate-limiter.ts          ← Token bucket rate limiter
├── retry.ts                 ← withRetry() — exponential backoff
├── cache.ts                 ← TtlCache — simple TTL cache
└── index.ts                 ← Public API barrel
```

## How to Create a New Adapter

When someone builds `@hippotask/adapter-jira` (or any platform adapter), they:

1. Create a new package: `packages/adapter-jira/`
2. Depend on `@hippotask/core` + `@hippotask/adapter-common`
3. Implement the `HippoAdapter<JiraConfig>` interface from `adapter.ts`
4. Use `mapStatus()` / `mapPriority()` for field normalization
5. Use `RateLimiter` and `withRetry()` for API call resilience
6. Use `TtlCache` if the platform has tight rate limits

**The interface is in `adapter.ts`** — read it carefully before implementing. Every method has JSDoc explaining the contract.

## Gotchas & Quirks

### StatusMap has two behaviors for unmapped statuses
`unmappedBehavior: "preserve"` uses the fallback status and keeps the raw value. `unmappedBehavior: "error"` throws. Most adapters should use `"preserve"` to be resilient to platform-specific statuses they don't know about.

### Priority scales are inverted between platforms
Linear: 1=Urgent, 4=Low. ClickUp: 1=Urgent, 4=Low. But the STRING representations differ. Always test with the actual platform priority values, not assumed numbers.

### `mapPriority` converts to string keys internally
Even if you pass a number `mapPriority(1, map)`, it does `String(1)` to look up in the map. Your `toHippo` keys must be strings: `{ "1": "urgent" }`.

### Error hierarchy extends HippoError from core
`AdapterError` → `HippoError` → `Error`. Subclasses (`RateLimitError`, etc.) use `override readonly code = "..."` to set their error codes, because `code` is `readonly` in the base class.

### `withRetry` has smart error classification
- 401/403 (`AuthError`) → **never** retried (surface immediately)
- 4xx (except 429) → **never** retried
- 429 (`RateLimitError`) → retried, respects `retryAfter`
- 5xx → retried with exponential backoff
- Network/timeout → retried

### RateLimiter uses token bucket algorithm
Tokens refill gradually over time, not all-at-once. `acquire()` returns a Promise that resolves when a token is available. The `available` getter shows current tokens.

### `HippoTaskCreate` vs `HippoTask`
`HippoTaskCreate` is what you pass TO an adapter to create a task (no id, no timestamps — the platform generates those). `HippoTask` is what comes BACK with all fields populated.

## Testing Commands

```bash
pnpm --filter @hippotask/adapter-common test
pnpm --filter @hippotask/adapter-common build
```

## When You Modify This Package

- If you change the `HippoAdapter` interface, **every** adapter must be updated. Be very careful with breaking changes.
- If you add a new error type, add it to both `errors.ts` AND `index.ts` exports.
- If you change mapping behavior, update the tests AND this AGENTS.md.
