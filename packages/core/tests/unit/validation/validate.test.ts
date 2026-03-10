import { describe, it, expect } from "vitest";
import { validateTask, validateProject } from "../../../src/validation/validate.js";

describe("validateTask", () => {
  const VALID_TASK = {
    id: "019470e1-3a4c-7f1c-8a5e-2b3c4d5e6f7a",
    title: "Fix bug",
    status: "todo",
    created_at: "2026-03-08T10:00:00Z",
    updated_at: "2026-03-08T10:00:00Z",
    schema_version: "1.0.0",
  };

  it("returns success for valid task", () => {
    const result = validateTask(VALID_TASK);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.title).toBe("Fix bug");
    }
  });

  it("returns error for invalid task", () => {
    const result = validateTask({ title: "", status: "banana" });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues.length).toBeGreaterThan(0);
    }
  });

  it("returns error with useful issue paths", () => {
    const result = validateTask({
      ...VALID_TASK,
      title: "",
      status: "invalid",
    });
    expect(result.success).toBe(false);
    if (!result.success) {
      const paths = result.error.issues.map((i) => i.path.join("."));
      expect(paths).toContain("title");
      expect(paths).toContain("status");
    }
  });

  it("validates completely unknown input safely", () => {
    const result = validateTask(null);
    expect(result.success).toBe(false);
  });

  it("validates string input safely", () => {
    const result = validateTask("not an object");
    expect(result.success).toBe(false);
  });

  it("validates number input safely", () => {
    const result = validateTask(42);
    expect(result.success).toBe(false);
  });
});

describe("validateProject", () => {
  const VALID_PROJECT = {
    id: "proj-123",
    name: "My Project",
    created_at: "2026-03-08T10:00:00Z",
    updated_at: "2026-03-08T10:00:00Z",
    schema_version: "1.0.0",
  };

  it("returns success for valid project", () => {
    const result = validateProject(VALID_PROJECT);
    expect(result.success).toBe(true);
  });

  it("returns error for invalid project", () => {
    const result = validateProject({ name: "" });
    expect(result.success).toBe(false);
  });
});
