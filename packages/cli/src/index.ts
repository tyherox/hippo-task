// @hippotask/cli — programmatic API
// Re-exports for consumers who want to use the store/safety layers directly

export { program, run } from "./cli.js";

// Store
export { FileTaskStore } from "./store.js";
export { resolveStorePath } from "./resolve-store.js";

// Safety layers
export {
  claimTask,
  releaseTask,
  isTaskClaimed,
  getClaimInfo,
  appendActivity,
  getActivityLog,
} from "./safety.js";
export type { ClaimInfo, ActivityEntry } from "./safety.js";

// File locking
export { acquireLock, releaseLock, isLockStale } from "./lock.js";

// Context
export { buildContext } from "./context.js";
export type { CliContext } from "./context.js";
