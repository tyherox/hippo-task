import type { HippoTask } from "@hippotask/core";
import { nowISO } from "@hippotask/core";

// ─── Metadata Keys ──────────────────────────────────────────────

const CLAIMED_BY_KEY = "hippotask.claimed_by";
const CLAIMED_AT_KEY = "hippotask.claimed_at";
const CLAIM_TTL_KEY = "hippotask.claim_ttl";
const ACTIVITY_KEY = "hippotask.activity";

/** Default claim TTL: 1 hour (in seconds). */
const DEFAULT_CLAIM_TTL = 3600;

// ─── Types ──────────────────────────────────────────────────────

export interface ClaimInfo {
  claimed_by: string;
  claimed_at: string;
  claim_ttl: number;
}

export interface ActivityEntry {
  agent_id: string;
  action: string;
  timestamp: string;
  detail?: string;
}

// ─── Layer 3: Task Claiming ─────────────────────────────────────

/**
 * Claim a task for an agent. Stores claim data in metadata.
 *
 * @throws Error if task is already claimed by a different agent (non-expired).
 */
export function claimTask(
  task: HippoTask,
  agentId: string,
  ttlSeconds: number = DEFAULT_CLAIM_TTL,
): HippoTask {
  const existing = getClaimInfo(task);

  // If already claimed by a different agent and not expired → reject
  if (existing !== null && existing.claimed_by !== agentId && !isClaimExpired(existing)) {
    throw new Error(
      `Task "${task.id}" is already claimed by "${existing.claimed_by}" ` +
        `(claimed at ${existing.claimed_at}, TTL ${existing.claim_ttl}s). ` +
        `Wait for the claim to expire or ask the owner to release it.`,
    );
  }

  const now = nowISO();
  const updated: HippoTask = {
    ...task,
    metadata: {
      ...task.metadata,
      [CLAIMED_BY_KEY]: agentId,
      [CLAIMED_AT_KEY]: now,
      [CLAIM_TTL_KEY]: ttlSeconds,
    },
  };

  // Log the claim action
  return appendActivity(updated, {
    agent_id: agentId,
    action: "claimed",
  });
}

/**
 * Release a task claim.
 *
 * Only the claiming agent can release (unless the claim has expired).
 * Releasing an unclaimed task is a no-op.
 */
export function releaseTask(task: HippoTask, agentId: string): HippoTask {
  const existing = getClaimInfo(task);

  // No claim → no-op
  if (existing === null) {
    return task;
  }

  // Different agent, non-expired → reject
  if (existing.claimed_by !== agentId && !isClaimExpired(existing)) {
    throw new Error(
      `Task "${task.id}" is not claimed by "${agentId}" — ` +
        `it's claimed by "${existing.claimed_by}".`,
    );
  }

  // Remove claim data from metadata
  const metadata = { ...task.metadata };
  delete metadata[CLAIMED_BY_KEY];
  delete metadata[CLAIMED_AT_KEY];
  delete metadata[CLAIM_TTL_KEY];

  const updated: HippoTask = { ...task, metadata };

  // Log the release action
  return appendActivity(updated, {
    agent_id: agentId,
    action: "released",
  });
}

/**
 * Check if a task is currently claimed (non-expired).
 */
export function isTaskClaimed(task: HippoTask): boolean {
  const info = getClaimInfo(task);
  if (info === null) return false;
  return !isClaimExpired(info);
}

/**
 * Get claim info for a task. Returns null if not claimed.
 */
export function getClaimInfo(task: HippoTask): ClaimInfo | null {
  const claimedBy = task.metadata?.[CLAIMED_BY_KEY];
  if (typeof claimedBy !== "string") return null;

  const claimedAt = task.metadata?.[CLAIMED_AT_KEY];
  const claimTtl = task.metadata?.[CLAIM_TTL_KEY];

  return {
    claimed_by: claimedBy,
    claimed_at: typeof claimedAt === "string" ? claimedAt : "",
    claim_ttl: typeof claimTtl === "number" ? claimTtl : DEFAULT_CLAIM_TTL,
  };
}

function isClaimExpired(info: ClaimInfo): boolean {
  if (info.claimed_at === "") return true;
  const claimedAtMs = new Date(info.claimed_at).getTime();
  const expiresAtMs = claimedAtMs + info.claim_ttl * 1000;
  return Date.now() > expiresAtMs;
}

// ─── Layer 4: Activity Log ──────────────────────────────────────

/**
 * Append an activity entry to a task's activity log.
 *
 * Stored at `metadata["hippotask.activity"]` as an array.
 * Preserves all other metadata fields.
 */
export function appendActivity(
  task: HippoTask,
  entry: Omit<ActivityEntry, "timestamp">,
): HippoTask {
  const log = getActivityLog(task);
  const fullEntry: ActivityEntry = {
    ...entry,
    timestamp: nowISO(),
  };
  log.push(fullEntry);

  return {
    ...task,
    metadata: {
      ...task.metadata,
      [ACTIVITY_KEY]: log,
    },
  };
}

/**
 * Get the activity log for a task.
 * Returns an empty array if no activity has been recorded.
 */
export function getActivityLog(task: HippoTask): ActivityEntry[] {
  const raw = task.metadata?.[ACTIVITY_KEY];
  if (!Array.isArray(raw)) return [];
  return [...raw] as ActivityEntry[];
}
