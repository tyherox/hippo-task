import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  claimTask,
  releaseTask,
  isTaskClaimed,
  getClaimInfo,
  appendActivity,
  getActivityLog,
} from "../../src/safety.js";
import { createTask } from "@hippotask/core";
import type { HippoTask } from "@hippotask/core";

describe("Task Claiming (Layer 3)", () => {
  let task: HippoTask;

  beforeEach(() => {
    task = createTask({ title: "Test task" });
  });

  describe("claimTask", () => {
    it("claims an unclaimed task", () => {
      const claimed = claimTask(task, "agent-1");
      const info = getClaimInfo(claimed);
      expect(info).not.toBeNull();
      expect(info?.claimed_by).toBe("agent-1");
      expect(info?.claimed_at).toBeTruthy();
      expect(info?.claim_ttl).toBe(3600); // default 1 hour
    });

    it("uses custom TTL", () => {
      const claimed = claimTask(task, "agent-1", 7200);
      const info = getClaimInfo(claimed);
      expect(info?.claim_ttl).toBe(7200);
    });

    it("throws if already claimed by another agent (non-expired)", () => {
      const claimed = claimTask(task, "agent-1");
      expect(() => claimTask(claimed, "agent-2")).toThrow(/already claimed/i);
    });

    it("allows re-claim by same agent", () => {
      const claimed = claimTask(task, "agent-1");
      const reclaimed = claimTask(claimed, "agent-1");
      expect(getClaimInfo(reclaimed)?.claimed_by).toBe("agent-1");
    });

    it("allows claim if previous claim expired", () => {
      // Claim with 0 TTL (immediately expired)
      const claimed = claimTask(task, "agent-1", 0);

      vi.useFakeTimers();
      vi.advanceTimersByTime(1000);

      const reclaimed = claimTask(claimed, "agent-2");
      expect(getClaimInfo(reclaimed)?.claimed_by).toBe("agent-2");

      vi.useRealTimers();
    });

    it("logs activity when claiming", () => {
      const claimed = claimTask(task, "agent-1");
      const log = getActivityLog(claimed);
      expect(log).toHaveLength(1);
      expect(log[0]?.action).toBe("claimed");
      expect(log[0]?.agent_id).toBe("agent-1");
    });
  });

  describe("releaseTask", () => {
    it("releases a claimed task", () => {
      const claimed = claimTask(task, "agent-1");
      const released = releaseTask(claimed, "agent-1");
      expect(isTaskClaimed(released)).toBe(false);
    });

    it("throws if releasing another agent's claim", () => {
      const claimed = claimTask(task, "agent-1");
      expect(() => releaseTask(claimed, "agent-2")).toThrow(/not claimed by/i);
    });

    it("allows release of expired claim by anyone", () => {
      const claimed = claimTask(task, "agent-1", 0);

      vi.useFakeTimers();
      vi.advanceTimersByTime(1000);

      const released = releaseTask(claimed, "agent-2");
      expect(isTaskClaimed(released)).toBe(false);

      vi.useRealTimers();
    });

    it("is a no-op on unclaimed tasks", () => {
      const released = releaseTask(task, "agent-1");
      expect(isTaskClaimed(released)).toBe(false);
    });

    it("logs activity when releasing", () => {
      const claimed = claimTask(task, "agent-1");
      const released = releaseTask(claimed, "agent-1");
      const log = getActivityLog(released);
      const releaseEntry = log.find((e) => e.action === "released");
      expect(releaseEntry).toBeTruthy();
      expect(releaseEntry?.agent_id).toBe("agent-1");
    });
  });

  describe("isTaskClaimed", () => {
    it("returns false for unclaimed task", () => {
      expect(isTaskClaimed(task)).toBe(false);
    });

    it("returns true for claimed task", () => {
      const claimed = claimTask(task, "agent-1");
      expect(isTaskClaimed(claimed)).toBe(true);
    });

    it("returns false for expired claim", () => {
      const claimed = claimTask(task, "agent-1", 0);

      vi.useFakeTimers();
      vi.advanceTimersByTime(1000);

      expect(isTaskClaimed(claimed)).toBe(false);

      vi.useRealTimers();
    });
  });

  describe("getClaimInfo", () => {
    it("returns null for unclaimed task", () => {
      expect(getClaimInfo(task)).toBeNull();
    });

    it("returns claim info for claimed task", () => {
      const claimed = claimTask(task, "agent-1", 3600);
      const info = getClaimInfo(claimed);
      expect(info).toEqual({
        claimed_by: "agent-1",
        claimed_at: expect.any(String),
        claim_ttl: 3600,
      });
    });
  });
});

describe("Activity Log (Layer 4)", () => {
  let task: HippoTask;

  beforeEach(() => {
    task = createTask({ title: "Test task" });
  });

  describe("appendActivity", () => {
    it("appends an entry to an empty activity log", () => {
      const updated = appendActivity(task, {
        agent_id: "agent-1",
        action: "created",
        detail: "Created via CLI",
      });
      const log = getActivityLog(updated);
      expect(log).toHaveLength(1);
      expect(log[0]).toEqual({
        agent_id: "agent-1",
        action: "created",
        timestamp: expect.any(String),
        detail: "Created via CLI",
      });
    });

    it("appends multiple entries in order", () => {
      let updated = appendActivity(task, { agent_id: "a1", action: "created" });
      updated = appendActivity(updated, { agent_id: "a1", action: "claimed" });
      updated = appendActivity(updated, {
        agent_id: "a1",
        action: "status_changed",
        detail: "todo → in_progress",
      });
      const log = getActivityLog(updated);
      expect(log).toHaveLength(3);
      expect(log.map((e) => e.action)).toEqual([
        "created",
        "claimed",
        "status_changed",
      ]);
    });

    it("preserves existing metadata", () => {
      const taskWithMeta = {
        ...task,
        metadata: { "jira.key": "PROJ-1", "custom.field": 42 },
      };
      const updated = appendActivity(taskWithMeta, {
        agent_id: "a1",
        action: "created",
      });
      expect(updated.metadata?.["jira.key"]).toBe("PROJ-1");
      expect(updated.metadata?.["custom.field"]).toBe(42);
    });
  });

  describe("getActivityLog", () => {
    it("returns empty array for task with no activity", () => {
      expect(getActivityLog(task)).toEqual([]);
    });

    it("returns activity entries in order", () => {
      let updated = appendActivity(task, { agent_id: "a1", action: "created" });
      updated = appendActivity(updated, { agent_id: "a2", action: "claimed" });
      const log = getActivityLog(updated);
      expect(log).toHaveLength(2);
      expect(log[0]?.agent_id).toBe("a1");
      expect(log[1]?.agent_id).toBe("a2");
    });
  });
});
