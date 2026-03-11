import { describe, it, expect } from "vitest";
import { HippoProjectSchema } from "../../../src/schema/project.js";

const VALID_PROJECT = {
  id: "019470e1-1111-7f1c-8a5e-000000000001",
  name: "HippoTask Core",
  created_at: "2026-03-01T10:00:00Z",
  updated_at: "2026-03-08T14:30:00Z",
  schema_version: "1.0.0",
};

describe("HippoProjectSchema", () => {
  it("accepts a minimal valid project", () => {
    const result = HippoProjectSchema.safeParse(VALID_PROJECT);
    expect(result.success).toBe(true);
  });

  it("accepts a full project", () => {
    const result = HippoProjectSchema.safeParse({
      ...VALID_PROJECT,
      external_ids: { jira: "PROJ", linear: "proj-abc" },
      description: "The core schema package",
      status: "active",
      metadata: { "jira.project_type": "software" },
    });
    expect(result.success).toBe(true);
  });

  it("accepts all valid project statuses", () => {
    for (const status of ["active", "paused", "completed", "archived"]) {
      const result = HippoProjectSchema.safeParse({ ...VALID_PROJECT, status });
      expect(result.success).toBe(true);
    }
  });

  it("rejects invalid project status", () => {
    const result = HippoProjectSchema.safeParse({ ...VALID_PROJECT, status: "deleted" });
    expect(result.success).toBe(false);
  });

  it("rejects missing name", () => {
    const { name, ...project } = VALID_PROJECT;
    const result = HippoProjectSchema.safeParse(project);
    expect(result.success).toBe(false);
  });

  it("rejects empty name", () => {
    const result = HippoProjectSchema.safeParse({ ...VALID_PROJECT, name: "" });
    expect(result.success).toBe(false);
  });

  it("rejects missing id", () => {
    const { id, ...project } = VALID_PROJECT;
    const result = HippoProjectSchema.safeParse(project);
    expect(result.success).toBe(false);
  });

  it("rejects missing schema_version", () => {
    const { schema_version, ...project } = VALID_PROJECT;
    const result = HippoProjectSchema.safeParse(project);
    expect(result.success).toBe(false);
  });

  it("rejects external_ids with empty string values", () => {
    const result = HippoProjectSchema.safeParse({
      ...VALID_PROJECT,
      external_ids: { jira: "" },
    });
    expect(result.success).toBe(false);
  });
});
