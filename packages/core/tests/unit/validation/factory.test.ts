import { describe, it, expect } from "vitest";
import { createTask, createProject } from "../../../src/validation/factory.js";
import { HippoTaskSchema } from "../../../src/schema/task.js";
import { HippoProjectSchema } from "../../../src/schema/project.js";

describe("createTask", () => {
  it("creates a valid task with just a title", () => {
    const task = createTask({ title: "My task" });

    // Should have auto-generated fields
    expect(task.id).toBeTruthy();
    expect(task.title).toBe("My task");
    expect(task.status).toBe("todo");
    expect(task.created_at).toBeTruthy();
    expect(task.updated_at).toBeTruthy();
    expect(task.schema_version).toBe("1.0.0");

    // Should be a valid HippoTask
    const validation = HippoTaskSchema.safeParse(task);
    expect(validation.success).toBe(true);
  });

  it("uses provided values over defaults", () => {
    const task = createTask({
      title: "Custom task",
      status: "in_progress",
      priority: "high",
      labels: ["important"],
    });

    expect(task.status).toBe("in_progress");
    expect(task.priority).toBe("high");
    expect(task.labels).toEqual(["important"]);
  });

  it("preserves custom id if provided", () => {
    const task = createTask({
      title: "With custom ID",
      id: "my-custom-id",
    });
    expect(task.id).toBe("my-custom-id");
  });

  it("auto-generates unique IDs", () => {
    const task1 = createTask({ title: "Task 1" });
    const task2 = createTask({ title: "Task 2" });
    expect(task1.id).not.toBe(task2.id);
  });

  it("preserves optional fields", () => {
    const task = createTask({
      title: "Detailed task",
      description: "Some description",
      due_date: "2026-03-15",
      assignees: [{ name: "Alice" }],
      estimate: 5,
      estimate_unit: "points",
      custom_fields: {
        sprint: { label: "Sprint", type: "string", value: "Sprint 1" },
      },
      metadata: { "jira.key": "PROJ-1" },
      external_ids: { jira: "PROJ-1" },
    });

    expect(task.description).toBe("Some description");
    expect(task.due_date).toBe("2026-03-15");
    expect(task.assignees).toHaveLength(1);
    expect(task.estimate).toBe(5);
    expect(task.custom_fields?.["sprint"]?.label).toBe("Sprint");
    expect(task.metadata?.["jira.key"]).toBe("PROJ-1");
    expect(task.external_ids?.["jira"]).toBe("PROJ-1");
  });

  it("sets created_at and updated_at to the same value", () => {
    const task = createTask({ title: "New task" });
    expect(task.created_at).toBe(task.updated_at);
  });
});

describe("createProject", () => {
  it("creates a valid project with just a name", () => {
    const project = createProject({ name: "My project" });

    expect(project.id).toBeTruthy();
    expect(project.name).toBe("My project");
    expect(project.created_at).toBeTruthy();
    expect(project.updated_at).toBeTruthy();
    expect(project.schema_version).toBe("1.0.0");

    const validation = HippoProjectSchema.safeParse(project);
    expect(validation.success).toBe(true);
  });

  it("uses provided values over defaults", () => {
    const project = createProject({
      name: "Custom project",
      status: "archived",
      description: "Archived project",
    });

    expect(project.status).toBe("archived");
    expect(project.description).toBe("Archived project");
  });

  it("auto-generates unique IDs", () => {
    const p1 = createProject({ name: "P1" });
    const p2 = createProject({ name: "P2" });
    expect(p1.id).not.toBe(p2.id);
  });
});
