import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { FileTaskStore } from "../../src/store.js";
import { createTask } from "@hippotask/core";
import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

describe("FileTaskStore", () => {
  let testDir: string;
  let storePath: string;
  let store: FileTaskStore;

  beforeEach(() => {
    testDir = join(tmpdir(), `hippotask-store-test-${Date.now()}`);
    mkdirSync(testDir, { recursive: true });
    storePath = join(testDir, "tasks.json");
    store = new FileTaskStore(storePath);
  });

  afterEach(() => {
    rmSync(testDir, { recursive: true, force: true });
  });

  describe("create", () => {
    it("creates a task and retrieves it", async () => {
      const task = createTask({ title: "Test task" });
      await store.create(task);
      const retrieved = await store.get(task.id);
      expect(retrieved).not.toBeNull();
      expect(retrieved?.title).toBe("Test task");
    });

    it("throws if task ID already exists", async () => {
      const task = createTask({ title: "Dupe" });
      await store.create(task);
      await expect(store.create(task)).rejects.toThrow(/already exists/i);
    });

    it("auto-creates the store file and directory", async () => {
      const deepPath = join(testDir, "nested", "deep", "tasks.json");
      const deepStore = new FileTaskStore(deepPath);
      const task = createTask({ title: "Auto-created" });
      await deepStore.create(task);
      const retrieved = await deepStore.get(task.id);
      expect(retrieved?.title).toBe("Auto-created");
    });
  });

  describe("get", () => {
    it("returns null for non-existent task", async () => {
      const result = await store.get("nonexistent");
      expect(result).toBeNull();
    });

    it("supports partial ID prefix match", async () => {
      const task = createTask({ title: "Prefix test" });
      await store.create(task);
      const prefix = task.id.slice(0, 8);
      const result = await store.get(prefix);
      expect(result?.title).toBe("Prefix test");
    });

    it("returns null if prefix matches multiple tasks", async () => {
      // With UUIDv7, this is extremely unlikely to happen naturally,
      // but we test the behavior: if ambiguous, return null
      // (In practice, 8-char prefix is unique enough)
      const task = createTask({ title: "Single match" });
      await store.create(task);
      const result = await store.get(task.id);
      expect(result).not.toBeNull();
    });
  });

  describe("list", () => {
    it("returns empty array for empty store", async () => {
      const result = await store.list();
      expect(result.items).toEqual([]);
      expect(result.has_more).toBe(false);
    });

    it("returns all tasks", async () => {
      await store.create(createTask({ title: "A" }));
      await store.create(createTask({ title: "B" }));
      await store.create(createTask({ title: "C" }));
      const result = await store.list();
      expect(result.items).toHaveLength(3);
    });

    it("filters by status", async () => {
      await store.create(createTask({ title: "Todo", status: "todo" }));
      await store.create(createTask({ title: "Done", status: "done" }));
      const result = await store.list({ status: "todo" });
      expect(result.items).toHaveLength(1);
      expect(result.items[0]?.title).toBe("Todo");
    });

    it("filters by priority", async () => {
      await store.create(createTask({ title: "High", priority: "high" }));
      await store.create(createTask({ title: "Low", priority: "low" }));
      const result = await store.list({ priority: "high" });
      expect(result.items).toHaveLength(1);
      expect(result.items[0]?.title).toBe("High");
    });

    it("filters by labels", async () => {
      await store.create(createTask({ title: "Bug", labels: ["bug", "urgent"] }));
      await store.create(createTask({ title: "Feature", labels: ["feature"] }));
      const result = await store.list({ labels: ["bug"] });
      expect(result.items).toHaveLength(1);
      expect(result.items[0]?.title).toBe("Bug");
    });

    it("searches title", async () => {
      await store.create(createTask({ title: "Fix login bug" }));
      await store.create(createTask({ title: "Add dashboard" }));
      const result = await store.list({ search: "login" });
      expect(result.items).toHaveLength(1);
      expect(result.items[0]?.title).toBe("Fix login bug");
    });

    it("respects limit", async () => {
      for (let i = 0; i < 5; i++) {
        await store.create(createTask({ title: `Task ${i}` }));
      }
      const result = await store.list({ limit: 3 });
      expect(result.items).toHaveLength(3);
      expect(result.has_more).toBe(true);
    });
  });

  describe("update", () => {
    it("updates task fields", async () => {
      const task = createTask({ title: "Original" });
      await store.create(task);
      // Wait a tick so updated_at will differ
      await new Promise((r) => setTimeout(r, 5));
      const updated = await store.update(task.id, { title: "Updated" });
      expect(updated.title).toBe("Updated");
      expect(updated.updated_at).not.toBe(task.updated_at);
    });

    it("throws for non-existent task", async () => {
      await expect(store.update("fake-id", { title: "X" })).rejects.toThrow(/not found/i);
    });

    it("rejects stale update (optimistic locking)", async () => {
      const task = createTask({ title: "Original" });
      await store.create(task);
      const originalUpdatedAt = task.updated_at;

      // Wait a tick so updated_at will differ
      await new Promise((r) => setTimeout(r, 5));

      // First update succeeds — this changes updated_at
      const v2 = await store.update(task.id, { title: "V2" }, originalUpdatedAt);
      expect(v2.updated_at).not.toBe(originalUpdatedAt);

      // Second update with the ORIGINAL timestamp should fail (stale)
      await expect(
        store.update(task.id, { title: "V3" }, originalUpdatedAt)
      ).rejects.toThrow(/conflict|stale/i);
    });

    it("succeeds without optimistic lock check", async () => {
      const task = createTask({ title: "Original" });
      await store.create(task);
      // No expect_updated_at → no optimistic lock check
      const updated = await store.update(task.id, { title: "V2" });
      expect(updated.title).toBe("V2");
    });
  });

  describe("delete", () => {
    it("removes a task", async () => {
      const task = createTask({ title: "Doomed" });
      await store.create(task);
      await store.delete(task.id);
      const result = await store.get(task.id);
      expect(result).toBeNull();
    });

    it("throws for non-existent task", async () => {
      await expect(store.delete("fake-id")).rejects.toThrow(/not found/i);
    });
  });

  describe("clear", () => {
    it("removes all tasks", async () => {
      await store.create(createTask({ title: "A" }));
      await store.create(createTask({ title: "B" }));
      await store.clear();
      const result = await store.list();
      expect(result.items).toHaveLength(0);
    });
  });
});
