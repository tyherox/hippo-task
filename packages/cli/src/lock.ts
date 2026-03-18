import {
  writeFileSync,
  readFileSync,
  unlinkSync,
  existsSync,
  mkdirSync,
} from "node:fs";
import { dirname } from "node:path";

interface LockData {
  locked_at: string;
  pid: number;
  ttl_ms: number;
}

interface AcquireOptions {
  /** Lock time-to-live in ms. Default: 30000 (30s). */
  ttlMs?: number;
  /** Max wait time in ms before giving up. Default: 5000 (5s). */
  waitMs?: number;
  /** Poll interval in ms. Default: 50. */
  pollMs?: number;
}

const DEFAULT_TTL_MS = 30_000;
const DEFAULT_WAIT_MS = 5_000;
const DEFAULT_POLL_MS = 50;

/**
 * Acquire a file lock. Waits if lock is held, breaks stale locks.
 *
 * The lock's TTL is stored in the lock file itself, so `isLockStale`
 * uses the TTL that was set when the lock was created.
 *
 * @throws Error if lock cannot be acquired within waitMs.
 */
export async function acquireLock(
  lockPath: string,
  options?: AcquireOptions,
): Promise<void> {
  const ttlMs = options?.ttlMs ?? DEFAULT_TTL_MS;
  const waitMs = options?.waitMs ?? DEFAULT_WAIT_MS;
  const pollMs = options?.pollMs ?? DEFAULT_POLL_MS;
  const deadline = Date.now() + waitMs;

  while (true) {
    // Try to acquire if no lock exists or existing lock is stale
    if (!existsSync(lockPath) || isLockStale(lockPath)) {
      mkdirSync(dirname(lockPath), { recursive: true });

      const data: LockData = {
        locked_at: new Date().toISOString(),
        pid: process.pid,
        ttl_ms: ttlMs,
      };

      try {
        writeFileSync(lockPath, JSON.stringify(data), { flag: "w" });
        return;
      } catch {
        // Race condition — fall through to retry
      }
    }

    // Lock is held and not stale — wait or give up
    if (Date.now() >= deadline) {
      throw new Error(
        `Failed to acquire lock: ${lockPath} is locked. ` +
          `Try again or delete the lock file if the owning process crashed.`,
      );
    }

    await new Promise((resolve) => setTimeout(resolve, pollMs));
  }
}

/**
 * Release a file lock.
 * Does not throw if the lock file doesn't exist.
 */
export async function releaseLock(lockPath: string): Promise<void> {
  try {
    if (existsSync(lockPath)) {
      unlinkSync(lockPath);
    }
  } catch {
    // Ignore — lock may have been released by another process
  }
}

/**
 * Check if a lock file is stale (older than its stored TTL).
 * Returns true if the lock doesn't exist or is expired.
 *
 * @param lockPath Path to the lock file.
 * @param ttlMsOverride Optional TTL override (used if lock file doesn't contain TTL).
 */
export function isLockStale(lockPath: string, ttlMsOverride?: number): boolean {
  try {
    if (!existsSync(lockPath)) return true;

    const content = readFileSync(lockPath, "utf-8");
    const data: LockData = JSON.parse(content);
    const lockedAt = new Date(data.locked_at).getTime();
    const ttl = data.ttl_ms ?? ttlMsOverride ?? DEFAULT_TTL_MS;
    return Date.now() - lockedAt > ttl;
  } catch {
    return true; // Corrupt or unreadable → treat as stale
  }
}
