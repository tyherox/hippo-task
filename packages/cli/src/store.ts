import {
  readFileSync,
  writeFileSync,
  existsSync,
  mkdirSync,
  renameSync,
} from "node:fs";
import { dirname, join } from "node:path";
import type { HippoTask, TaskQuery, PaginatedResult } from "@hippotask/core";
import { nowISO } from "@hippotask/core";
import { acquireLock, releaseLock } from "./lock.js";

interface StoreData {
  tasks: HippoTask[];
}

/**
 * File-based task store.
 *
 * Stores tasks as a JSON file. Supports:
 * - Atomic writes (write-to-temp + rename)
 * - File locking (Layer 2) to serialize concurrent access
 * - Optimistic locking (Layer 1) via `expect_updated_at` parameter
 * - Auto-creates directory if missing
 */
export class FileTaskStore {
  private readonly filePath: string;
  private readonly lockPath: string;

  constructor(filePath: string) {
    this.filePath = filePath;
    this.lockPath = filePath.replace(/\.json$/, ".lock");
  }

  /** Get the store file path. */
  get path(): string {
    return this.filePath;
  }

  /** Get a task by ID or ID prefix. Returns null if not found or ambiguous. */
  async get(id: string): Promise<HippoTask | null> {
    const data = this.readStore();
    // Exact match first
    const exact = data.tasks.find((t) => t.id === id);
    if (exact) return exact;
    // Prefix match
    const matches = data.tasks.filter((t) => t.id.startsWith(id));
    if (matches.length === 1) return matches[0] ?? null;
    return null;
  }

  /** List tasks with optional filters. */
  async list(query?: TaskQuery): Promise<PaginatedResult<HippoTask>> {
    const data = this.readStore();
    let tasks = [...data.tasks];

    if (query) {
      tasks = this.applyFilters(tasks, query);
    }

    // Sort by updated_at desc by default
    tasks.sort((a, b) => b.updated_at.localeCompare(a.updated_at));

    const limit = query?.limit ?? 50;
    const hasMore = tasks.length > limit;
    const items = tasks.slice(0, limit);

    return { items, has_more: hasMore };
  }

  /** Create a task. Throws if ID already exists. */
  async create(task: HippoTask): Promise<HippoTask> {
    await acquireLock(this.lockPath);
    try {
      const data = this.readStore();
      if (data.tasks.some((t) => t.id === task.id)) {
        throw new Error(`Task "${task.id}" already exists`);
      }
      data.tasks.push(task);
      this.writeStore(data);
      return task;
    } finally {
      await releaseLock(this.lockPath);
    }
  }

  /**
   * Update a task by ID.
   *
   * @param id - Task ID (exact or prefix match)
   * @param updates - Partial task fields to update
   * @param expectUpdatedAt - If provided, rejects if the task's updated_at
   *   doesn't match (optimistic locking, Layer 1)
   * @throws Error if task not found or optimistic lock conflict
   */
  async update(
    id: string,
    updates: Partial<HippoTask>,
    expectUpdatedAt?: string,
  ): Promise<HippoTask> {
    await acquireLock(this.lockPath);
    try {
      const data = this.readStore();
      const index = this.findTaskIndex(data.tasks, id);
      if (index === -1) {
        throw new Error(`Task not found: "${id}"`);
      }

      const existing = data.tasks[index]!;

      // Layer 1: Optimistic locking
      if (expectUpdatedAt != null && existing.updated_at !== expectUpdatedAt) {
        throw new Error(
          `Conflict: task "${id}" has been modified since you last read it. ` +
            `Expected updated_at="${expectUpdatedAt}", ` +
            `actual="${existing.updated_at}". Re-read and retry.`,
        );
      }

      const updated: HippoTask = {
        ...existing,
        ...updates,
        id: existing.id, // Never change ID
        created_at: existing.created_at, // Never change created_at
        updated_at: nowISO(),
        schema_version: existing.schema_version,
      };

      data.tasks[index] = updated;
      this.writeStore(data);
      return updated;
    } finally {
      await releaseLock(this.lockPath);
    }
  }

  /** Delete a task by ID. Throws if not found. */
  async delete(id: string): Promise<void> {
    await acquireLock(this.lockPath);
    try {
      const data = this.readStore();
      const index = this.findTaskIndex(data.tasks, id);
      if (index === -1) {
        throw new Error(`Task not found: "${id}"`);
      }
      data.tasks.splice(index, 1);
      this.writeStore(data);
    } finally {
      await releaseLock(this.lockPath);
    }
  }

  /** Remove all tasks. */
  async clear(): Promise<void> {
    await acquireLock(this.lockPath);
    try {
      this.writeStore({ tasks: [] });
    } finally {
      await releaseLock(this.lockPath);
    }
  }

  /** Get total task count (without reading all tasks into filters). */
  async count(): Promise<number> {
    const data = this.readStore();
    return data.tasks.length;
  }

  // ─── Private ──────────────────────────────────────────────

  private readStore(): StoreData {
    try {
      if (!existsSync(this.filePath)) {
        return { tasks: [] };
      }
      const content = readFileSync(this.filePath, "utf-8");
      const parsed = JSON.parse(content) as StoreData;
      return { tasks: Array.isArray(parsed.tasks) ? parsed.tasks : [] };
    } catch {
      return { tasks: [] };
    }
  }

  private writeStore(data: StoreData): void {
    const dir = dirname(this.filePath);
    mkdirSync(dir, { recursive: true });

    // Atomic write: write to temp file, then rename
    const tempPath = `${this.filePath}.tmp.${process.pid}`;
    writeFileSync(tempPath, JSON.stringify(data, null, 2) + "\n");
    renameSync(tempPath, this.filePath);
  }

  private findTaskIndex(tasks: HippoTask[], id: string): number {
    // Exact match first
    const exactIdx = tasks.findIndex((t) => t.id === id);
    if (exactIdx !== -1) return exactIdx;

    // Prefix match
    const matches: number[] = [];
    for (let i = 0; i < tasks.length; i++) {
      if (tasks[i]!.id.startsWith(id)) {
        matches.push(i);
      }
    }
    return matches.length === 1 ? matches[0]! : -1;
  }

  private applyFilters(tasks: HippoTask[], query: TaskQuery): HippoTask[] {
    let result = tasks;

    if (query.status != null) {
      const statuses = Array.isArray(query.status) ? query.status : [query.status];
      result = result.filter((t) => statuses.includes(t.status));
    }

    if (query.priority != null) {
      result = result.filter((t) => t.priority === query.priority);
    }

    if (query.project_id != null) {
      result = result.filter((t) => t.project_id === query.project_id);
    }

    if (query.labels != null && query.labels.length > 0) {
      result = result.filter((t) =>
        query.labels!.some((label) => t.labels?.includes(label)),
      );
    }

    if (query.assignee != null) {
      result = result.filter((t) =>
        t.assignees?.some(
          (a) => a.email === query.assignee || a.id === query.assignee,
        ),
      );
    }

    if (query.search != null && query.search.length > 0) {
      const searchLower = query.search.toLowerCase();
      result = result.filter(
        (t) =>
          t.title.toLowerCase().includes(searchLower) ||
          t.description?.toLowerCase().includes(searchLower),
      );
    }

    if (query.updated_since != null) {
      result = result.filter((t) => t.updated_at >= query.updated_since!);
    }

    return result;
  }
}
