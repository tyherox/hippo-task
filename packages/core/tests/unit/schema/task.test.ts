import { describe, expect, it } from "vitest";
import { HippoTaskSchema } from "../../../src/schema/task.js";

const VALID_MINIMAL_TASK = {
  id: "019470e1-3a4c-7f1c-8a5e-2b3c4d5e6f7a",
  title: "Fix login bug",
  status: "todo",
  created_at: "2026-03-08T10:00:00Z",
  updated_at: "2026-03-08T10:00:00Z",
  schema_version: "1.0.0",
};

const VALID_FULL_TASK = {
  ...VALID_MINIMAL_TASK,
  external_ids: { jira: "PROJ-123", linear: "ENG-456" },
  description: "## Fix the login\n\n- Reset flow\n- Error handling",
  description_format: "markdown",
  status_raw: "To Do",
  priority: "high",
  priority_raw: "2",
  assignees: [
    {
      name: "Alice",
      email: "alice@example.com",
      external_ids: { jira: "u123" },
    },
  ],
  creator: { name: "Bob", email: "bob@example.com" },
  due_date: "2026-03-15",
  start_date: "2026-03-01",
  completed_at: undefined,
  project_id: "019470e1-1111-7f1c-8a5e-000000000001",
  parent_id: "019470e1-2222-7f1c-8a5e-000000000002",
  labels: ["auth", "backend"],
  estimate: 8,
  estimate_unit: "points",
  custom_fields: {
    sprint: { label: "Sprint", type: "string", value: "Sprint 42" },
  },
  metadata: {
    "jira.issue_type": "Story",
    "linear.team_key": "ENG",
  },
};

describe("HippoTaskSchema", () => {
  describe("valid tasks", () => {
    it("accepts a minimal valid task", () => {
      const result = HippoTaskSchema.safeParse(VALID_MINIMAL_TASK);
      expect(result.success).toBe(true);
    });

    it("accepts a full valid task", () => {
      const result = HippoTaskSchema.safeParse(VALID_FULL_TASK);
      expect(result.success).toBe(true);
    });

    it("accepts all valid statuses", () => {
      for (const status of [
        "backlog",
        "todo",
        "in_progress",
        "in_review",
        "done",
        "cancelled",
      ]) {
        const result = HippoTaskSchema.safeParse({
          ...VALID_MINIMAL_TASK,
          status,
        });
        expect(result.success).toBe(true);
      }
    });

    it("accepts all valid priorities", () => {
      for (const priority of ["none", "low", "medium", "high", "urgent"]) {
        const result = HippoTaskSchema.safeParse({
          ...VALID_MINIMAL_TASK,
          priority,
        });
        expect(result.success).toBe(true);
      }
    });

    it("accepts priority_raw as string", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        priority: "high",
        priority_raw: "High",
      });
      expect(result.success).toBe(true);
    });

    it("accepts priority_raw as number", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        priority: "urgent",
        priority_raw: 1,
      });
      expect(result.success).toBe(true);
    });

    it("accepts multiple assignees", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        assignees: [
          { name: "Alice" },
          { name: "Bob" },
          { email: "charlie@example.com" },
        ],
      });
      expect(result.success).toBe(true);
    });

    it("accepts empty labels array", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        labels: [],
      });
      expect(result.success).toBe(true);
    });

    it("accepts date-only due_date", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        due_date: "2026-03-15",
      });
      expect(result.success).toBe(true);
    });

    it("accepts datetime due_date", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        due_date: "2026-03-15T17:00:00Z",
      });
      expect(result.success).toBe(true);
    });
  });

  describe("required field validation", () => {
    it("rejects missing id", () => {
      const { id, ...task } = VALID_MINIMAL_TASK;
      const result = HippoTaskSchema.safeParse(task);
      expect(result.success).toBe(false);
    });

    it("rejects missing title", () => {
      const { title, ...task } = VALID_MINIMAL_TASK;
      const result = HippoTaskSchema.safeParse(task);
      expect(result.success).toBe(false);
    });

    it("rejects empty title", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        title: "",
      });
      expect(result.success).toBe(false);
    });

    it("rejects whitespace-only title", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        title: "   ",
      });
      expect(result.success).toBe(false);
    });

    it("rejects missing status", () => {
      const { status, ...task } = VALID_MINIMAL_TASK;
      const result = HippoTaskSchema.safeParse(task);
      expect(result.success).toBe(false);
    });

    it("rejects invalid status", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        status: "banana",
      });
      expect(result.success).toBe(false);
    });

    it("rejects missing created_at", () => {
      const { created_at, ...task } = VALID_MINIMAL_TASK;
      const result = HippoTaskSchema.safeParse(task);
      expect(result.success).toBe(false);
    });

    it("rejects missing updated_at", () => {
      const { updated_at, ...task } = VALID_MINIMAL_TASK;
      const result = HippoTaskSchema.safeParse(task);
      expect(result.success).toBe(false);
    });

    it("rejects missing schema_version", () => {
      const { schema_version, ...task } = VALID_MINIMAL_TASK;
      const result = HippoTaskSchema.safeParse(task);
      expect(result.success).toBe(false);
    });
  });

  describe("date format validation", () => {
    it("rejects non-ISO created_at", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        created_at: "yesterday",
      });
      expect(result.success).toBe(false);
    });

    it("rejects date-only created_at (datetime required)", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        created_at: "2026-03-08",
      });
      expect(result.success).toBe(false);
    });

    it("accepts created_at with a timezone offset", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        created_at: "2026-03-08T10:00:00+05:30",
      });
      expect(result.success).toBe(true);
    });

    it("rejects non-ISO updated_at", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        updated_at: "last tuesday",
      });
      expect(result.success).toBe(false);
    });

    it("rejects non-ISO due_date", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        due_date: "next friday",
      });
      expect(result.success).toBe(false);
    });

    it("rejects an impossible calendar date", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        due_date: "2026-02-30",
      });
      expect(result.success).toBe(false);
    });

    it("accepts datetime completed_at and rejects garbage", () => {
      expect(
        HippoTaskSchema.safeParse({
          ...VALID_MINIMAL_TASK,
          completed_at: "2026-03-09T08:00:00Z",
        }).success,
      ).toBe(true);
      expect(
        HippoTaskSchema.safeParse({
          ...VALID_MINIMAL_TASK,
          completed_at: "done",
        }).success,
      ).toBe(false);
    });
  });

  describe("business rules", () => {
    it("rejects start_date after due_date", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        start_date: "2026-03-20",
        due_date: "2026-03-10",
      });
      expect(result.success).toBe(false);
    });

    it("accepts start_date equal to due_date", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        start_date: "2026-03-10",
        due_date: "2026-03-10",
      });
      expect(result.success).toBe(true);
    });

    it("accepts start_date before due_date", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        start_date: "2026-03-01",
        due_date: "2026-03-15",
      });
      expect(result.success).toBe(true);
    });

    it("accepts a datetime start_date with a date-only due_date on the same day", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        start_date: "2026-03-10T09:00:00Z",
        due_date: "2026-03-10",
      });
      expect(result.success).toBe(true);
    });

    it("compares start/due as instants across timezone offsets", () => {
      // 2026-03-08T23:00:00-05:00 is 2026-03-09T04:00:00Z — later than the
      // due datetime despite sorting earlier lexicographically.
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        start_date: "2026-03-08T23:00:00-05:00",
        due_date: "2026-03-09T01:00:00Z",
      });
      expect(result.success).toBe(false);
    });

    it("rejects negative estimate", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        estimate: -1,
      });
      expect(result.success).toBe(false);
    });

    it("rejects zero estimate", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        estimate: 0,
      });
      expect(result.success).toBe(false);
    });

    it("accepts positive estimate", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        estimate: 0.5,
      });
      expect(result.success).toBe(true);
    });

    it("rejects non-finite estimate", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        estimate: Number.POSITIVE_INFINITY,
      });
      expect(result.success).toBe(false);
    });

    it("rejects non-finite priority_raw", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        priority_raw: Number.POSITIVE_INFINITY,
      });
      expect(result.success).toBe(false);
    });

    it("rejects estimate_unit without estimate", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        estimate_unit: "hours",
      });
      expect(result.success).toBe(false);
    });

    it("rejects a task that is its own parent", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        parent_id: VALID_MINIMAL_TASK.id,
      });
      expect(result.success).toBe(false);
    });

    it("rejects empty-string labels", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        labels: ["auth", ""],
      });
      expect(result.success).toBe(false);
    });

    it("rejects empty project_id and parent_id", () => {
      expect(
        HippoTaskSchema.safeParse({ ...VALID_MINIMAL_TASK, project_id: "" })
          .success,
      ).toBe(false);
      expect(
        HippoTaskSchema.safeParse({ ...VALID_MINIMAL_TASK, parent_id: "" })
          .success,
      ).toBe(false);
    });
  });

  describe("external_ids", () => {
    it("rejects external_ids with empty string values", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        external_ids: { jira: "" },
      });
      expect(result.success).toBe(false);
    });

    it("accepts external_ids with non-empty string values", () => {
      const result = HippoTaskSchema.safeParse({
        ...VALID_MINIMAL_TASK,
        external_ids: { jira: "PROJ-123", linear: "ENG-456" },
      });
      expect(result.success).toBe(true);
    });

    it("rejects empty-string record keys", () => {
      expect(
        HippoTaskSchema.safeParse({
          ...VALID_MINIMAL_TASK,
          external_ids: { "": "PROJ-123" },
        }).success,
      ).toBe(false);
      expect(
        HippoTaskSchema.safeParse({
          ...VALID_MINIMAL_TASK,
          metadata: { "": "orphan value" },
        }).success,
      ).toBe(false);
    });
  });

  describe("type inference", () => {
    it("inferred type has correct required fields", () => {
      const result = HippoTaskSchema.safeParse(VALID_MINIMAL_TASK);
      if (result.success) {
        const task = result.data;
        // These should be accessible without optional chaining
        const _id: string = task.id;
        const _title: string = task.title;
        const _status: string = task.status;
        const _created: string = task.created_at;
        const _updated: string = task.updated_at;
        const _version: string = task.schema_version;
        // Prevent unused warnings
        expect([
          _id,
          _title,
          _status,
          _created,
          _updated,
          _version,
        ]).toBeTruthy();
      }
    });
  });
});
