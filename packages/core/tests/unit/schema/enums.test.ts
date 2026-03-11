import { describe, it, expect } from "vitest";
import {
  HippoStatusSchema,
  HippoPrioritySchema,
  DescriptionFormatSchema,
  EstimateUnitSchema,
  ProjectStatusSchema,
  CustomFieldTypeSchema,
  HIPPO_STATUSES,
  HIPPO_PRIORITIES,
} from "../../../src/schema/enums.js";

describe("HippoStatusSchema", () => {
  const validStatuses = ["backlog", "todo", "in_progress", "in_review", "done", "cancelled"];

  it.each(validStatuses)("accepts valid status: %s", (status) => {
    expect(HippoStatusSchema.safeParse(status).success).toBe(true);
  });

  it("rejects invalid status", () => {
    const result = HippoStatusSchema.safeParse("banana");
    expect(result.success).toBe(false);
  });

  it("rejects empty string", () => {
    expect(HippoStatusSchema.safeParse("").success).toBe(false);
  });

  it("rejects non-string", () => {
    expect(HippoStatusSchema.safeParse(42).success).toBe(false);
  });

  it("exports HIPPO_STATUSES array with all values", () => {
    expect(HIPPO_STATUSES).toEqual(validStatuses);
  });
});

describe("HippoPrioritySchema", () => {
  const validPriorities = ["none", "low", "medium", "high", "urgent"];

  it.each(validPriorities)("accepts valid priority: %s", (priority) => {
    expect(HippoPrioritySchema.safeParse(priority).success).toBe(true);
  });

  it("rejects invalid priority", () => {
    expect(HippoPrioritySchema.safeParse("critical").success).toBe(false);
  });

  it("exports HIPPO_PRIORITIES array with all values", () => {
    expect(HIPPO_PRIORITIES).toEqual(validPriorities);
  });
});

describe("DescriptionFormatSchema", () => {
  it.each(["markdown", "plaintext", "html"])("accepts: %s", (format) => {
    expect(DescriptionFormatSchema.safeParse(format).success).toBe(true);
  });

  it("rejects invalid format", () => {
    expect(DescriptionFormatSchema.safeParse("adf").success).toBe(false);
  });
});

describe("EstimateUnitSchema", () => {
  it.each(["points", "hours", "minutes"])("accepts: %s", (unit) => {
    expect(EstimateUnitSchema.safeParse(unit).success).toBe(true);
  });

  it("rejects invalid unit", () => {
    expect(EstimateUnitSchema.safeParse("days").success).toBe(false);
  });
});

describe("ProjectStatusSchema", () => {
  it.each(["active", "paused", "completed", "archived"])("accepts: %s", (status) => {
    expect(ProjectStatusSchema.safeParse(status).success).toBe(true);
  });

  it("rejects invalid status", () => {
    expect(ProjectStatusSchema.safeParse("deleted").success).toBe(false);
  });
});

describe("CustomFieldTypeSchema", () => {
  const validTypes = [
    "string", "number", "boolean", "date",
    "select", "multi_select", "url", "email", "person",
  ];

  it.each(validTypes)("accepts: %s", (type) => {
    expect(CustomFieldTypeSchema.safeParse(type).success).toBe(true);
  });

  it("rejects invalid type", () => {
    expect(CustomFieldTypeSchema.safeParse("array").success).toBe(false);
  });
});
