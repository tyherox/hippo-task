import { describe, it, expect } from "vitest";
import { TaskChangeEventSchema } from "../../../src/schema/events.js";

describe("TaskChangeEventSchema", () => {
  it("accepts a created event with task", () => {
    const result = TaskChangeEventSchema.safeParse({
      type: "created",
      platform: "jira",
      external_id: "PROJ-123",
      task: {
        id: "abc-123",
        title: "New task",
        status: "todo",
        created_at: "2026-03-08T10:00:00Z",
        updated_at: "2026-03-08T10:00:00Z",
        schema_version: "1.0.0",
      },
      timestamp: "2026-03-08T10:00:00Z",
    });
    expect(result.success).toBe(true);
  });

  it("accepts an updated event", () => {
    const result = TaskChangeEventSchema.safeParse({
      type: "updated",
      platform: "linear",
      external_id: "ENG-456",
      timestamp: "2026-03-08T11:00:00Z",
    });
    expect(result.success).toBe(true);
  });

  it("accepts a deleted event without task", () => {
    const result = TaskChangeEventSchema.safeParse({
      type: "deleted",
      platform: "github",
      external_id: "owner/repo#42",
      timestamp: "2026-03-08T12:00:00Z",
    });
    expect(result.success).toBe(true);
  });

  it("rejects invalid type", () => {
    const result = TaskChangeEventSchema.safeParse({
      type: "archived",
      platform: "jira",
      external_id: "PROJ-123",
      timestamp: "2026-03-08T10:00:00Z",
    });
    expect(result.success).toBe(false);
  });

  it("rejects missing platform", () => {
    const result = TaskChangeEventSchema.safeParse({
      type: "created",
      external_id: "PROJ-123",
      timestamp: "2026-03-08T10:00:00Z",
    });
    expect(result.success).toBe(false);
  });

  it("rejects missing external_id", () => {
    const result = TaskChangeEventSchema.safeParse({
      type: "created",
      platform: "jira",
      timestamp: "2026-03-08T10:00:00Z",
    });
    expect(result.success).toBe(false);
  });

  it("rejects missing timestamp", () => {
    const result = TaskChangeEventSchema.safeParse({
      type: "created",
      platform: "jira",
      external_id: "PROJ-123",
    });
    expect(result.success).toBe(false);
  });
});
