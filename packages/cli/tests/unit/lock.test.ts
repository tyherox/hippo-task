import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { acquireLock, releaseLock, isLockStale } from "../../src/lock.js";
import { mkdirSync, rmSync, existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

describe("File Locking", () => {
  let lockDir: string;
  let lockPath: string;

  beforeEach(() => {
    lockDir = join(tmpdir(), `hippotask-lock-test-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`);
    mkdirSync(lockDir, { recursive: true });
    lockPath = join(lockDir, "tasks.lock");
  });

  afterEach(() => {
    rmSync(lockDir, { recursive: true, force: true });
  });

  describe("acquireLock", () => {
    it("creates a lock file", async () => {
      await acquireLock(lockPath);
      expect(existsSync(lockPath)).toBe(true);
    });

    it("lock file contains timestamp and pid and ttl", async () => {
      await acquireLock(lockPath);
      const content = readFileSync(lockPath, "utf-8");
      const data = JSON.parse(content);
      expect(data.locked_at).toBeTruthy();
      expect(data.pid).toBeTruthy();
      expect(data.ttl_ms).toBeTruthy();
    });

    it("throws if lock already held (non-stale)", async () => {
      // Acquire with long TTL so it won't be stale
      await acquireLock(lockPath, { ttlMs: 60_000 });
      // Second acquire should fail fast
      await expect(
        acquireLock(lockPath, { ttlMs: 60_000, waitMs: 200, pollMs: 50 }),
      ).rejects.toThrow(/locked/i);
    });

    it("acquires lock if existing lock is stale (short TTL expired)", async () => {
      // Acquire with very short TTL
      await acquireLock(lockPath, { ttlMs: 10 });
      // Wait for the lock's own TTL to expire
      await new Promise((r) => setTimeout(r, 50));
      // Now the lock is stale based on its stored ttl_ms=10
      // New acquire should succeed
      await acquireLock(lockPath, { ttlMs: 60_000, waitMs: 500, pollMs: 20 });
      expect(existsSync(lockPath)).toBe(true);
    });
  });

  describe("releaseLock", () => {
    it("removes the lock file", async () => {
      await acquireLock(lockPath);
      await releaseLock(lockPath);
      expect(existsSync(lockPath)).toBe(false);
    });

    it("does not throw if lock file missing", async () => {
      await expect(releaseLock(lockPath)).resolves.not.toThrow();
    });
  });

  describe("isLockStale", () => {
    it("returns true for expired lock", async () => {
      await acquireLock(lockPath, { ttlMs: 10 });
      await new Promise((r) => setTimeout(r, 50));
      expect(isLockStale(lockPath)).toBe(true);
    });

    it("returns false for fresh lock", async () => {
      await acquireLock(lockPath, { ttlMs: 60_000 });
      expect(isLockStale(lockPath)).toBe(false);
    });

    it("returns true if lock file missing", () => {
      expect(isLockStale(lockPath)).toBe(true);
    });
  });
});
