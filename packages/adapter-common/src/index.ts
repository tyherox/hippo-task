// ─── @hippotask/adapter-common ───────────────────────────────────
// Shared adapter interfaces, types, and utilities.
// ─────────────────────────────────────────────────────────────────

// ─── Interfaces ─────────────────────────────────────────────────
export type {
  HippoAdapter,
  BaseAdapterConfig,
  AdapterCapabilities,
} from "./adapter.js";

// ─── Types ──────────────────────────────────────────────────────
export type { HippoTaskCreate, HippoTaskUpdate } from "./types.js";

// ─── Status Mapping ─────────────────────────────────────────────
export type { StatusMap } from "./status-map.js";
export { mapStatus, mapStatusReverse } from "./status-map.js";

// ─── Priority Mapping ───────────────────────────────────────────
export type { PriorityMap } from "./priority-map.js";
export { mapPriority, mapPriorityReverse } from "./priority-map.js";

// ─── Errors ─────────────────────────────────────────────────────
export {
  AdapterError,
  RateLimitError,
  NotFoundError,
  AuthError,
} from "./errors.js";

// ─── Utilities ──────────────────────────────────────────────────
export { RateLimiter } from "./rate-limiter.js";
export { withRetry } from "./retry.js";
export type { RetryOptions } from "./retry.js";
export { TtlCache } from "./cache.js";
