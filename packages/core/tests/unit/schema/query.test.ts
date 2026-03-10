import { describe, it, expect } from "vitest";
import { TaskQuerySchema, PaginatedResultSchema } from "../../../src/schema/query.js";
import { HippoTaskSchema } from "../../../src/schema/task.js";

describe("TaskQuerySchema", () => {
  it("accepts an empty query (all defaults)", () => {
    const result = TaskQuerySchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("accepts a full query", () => {
    const result = TaskQuerySchema.safeParse({
      project_id: "proj-123",
      status: "in_progress",
      assignee: "alice@example.com",
      labels: ["bug", "urgent"],
      priority: "high",
      updated_since: "2026-03-01T00:00:00Z",
      search: "login",
      limit: 25,
      cursor: "cursor-abc",
      sort_by: "due_date",
      sort_direction: "asc",
    });
    expect(result.success).toBe(true);
  });

  it("accepts status as array", () => {
    const result = TaskQuerySchema.safeParse({
      status: ["todo", "in_progress"],
    });
    expect(result.success).toBe(true);
  });

  it("accepts status as single string", () => {
    const result = TaskQuerySchema.safeParse({ status: "done" });
    expect(result.success).toBe(true);
  });

  it("rejects invalid status in array", () => {
    const result = TaskQuerySchema.safeParse({
      status: ["todo", "banana"],
    });
    expect(result.success).toBe(false);
  });

  it("rejects limit > 200", () => {
    const result = TaskQuerySchema.safeParse({ limit: 201 });
    expect(result.success).toBe(false);
  });

  it("rejects limit < 1", () => {
    const result = TaskQuerySchema.safeParse({ limit: 0 });
    expect(result.success).toBe(false);
  });

  it("rejects invalid sort_by", () => {
    const result = TaskQuerySchema.safeParse({ sort_by: "assignee" });
    expect(result.success).toBe(false);
  });

  it("rejects invalid sort_direction", () => {
    const result = TaskQuerySchema.safeParse({ sort_direction: "random" });
    expect(result.success).toBe(false);
  });
});

describe("PaginatedResultSchema", () => {
  it("accepts a valid paginated result", () => {
    const result = PaginatedResultSchema.safeParse({
      items: [],
      has_more: false,
    });
    expect(result.success).toBe(true);
  });

  it("accepts with total and cursor", () => {
    const result = PaginatedResultSchema.safeParse({
      items: [{ some: "data" }],
      total: 100,
      next_cursor: "abc123",
      has_more: true,
    });
    expect(result.success).toBe(true);
  });

  it("accepts null next_cursor", () => {
    const result = PaginatedResultSchema.safeParse({
      items: [],
      next_cursor: null,
      has_more: false,
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing items", () => {
    const result = PaginatedResultSchema.safeParse({
      has_more: false,
    });
    expect(result.success).toBe(false);
  });

  it("rejects missing has_more", () => {
    const result = PaginatedResultSchema.safeParse({
      items: [],
    });
    expect(result.success).toBe(false);
  });
});
