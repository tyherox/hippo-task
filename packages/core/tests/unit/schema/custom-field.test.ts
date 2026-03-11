import { describe, it, expect } from "vitest";
import { HippoCustomFieldSchema } from "../../../src/schema/custom-field.js";

describe("HippoCustomFieldSchema", () => {
  it("accepts a string custom field", () => {
    const result = HippoCustomFieldSchema.safeParse({
      label: "Sprint",
      type: "string",
      value: "Sprint 42",
    });
    expect(result.success).toBe(true);
  });

  it("accepts a number custom field", () => {
    const result = HippoCustomFieldSchema.safeParse({
      label: "Story Points",
      type: "number",
      value: 8,
    });
    expect(result.success).toBe(true);
  });

  it("accepts a select custom field with options", () => {
    const result = HippoCustomFieldSchema.safeParse({
      label: "Environment",
      type: "select",
      value: "production",
      options: ["development", "staging", "production"],
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.options).toEqual(["development", "staging", "production"]);
    }
  });

  it("accepts a boolean custom field", () => {
    const result = HippoCustomFieldSchema.safeParse({
      label: "Is Blocked",
      type: "boolean",
      value: true,
    });
    expect(result.success).toBe(true);
  });

  it("accepts a person custom field", () => {
    const result = HippoCustomFieldSchema.safeParse({
      label: "Reviewed By",
      type: "person",
      value: { name: "Jane", email: "jane@example.com" },
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing label", () => {
    const result = HippoCustomFieldSchema.safeParse({
      type: "string",
      value: "hello",
    });
    expect(result.success).toBe(false);
  });

  it("rejects empty label", () => {
    const result = HippoCustomFieldSchema.safeParse({
      label: "",
      type: "string",
      value: "hello",
    });
    expect(result.success).toBe(false);
  });

  it("rejects invalid type", () => {
    const result = HippoCustomFieldSchema.safeParse({
      label: "Foo",
      type: "array",
      value: [],
    });
    expect(result.success).toBe(false);
  });

  it("accepts missing options for non-select types", () => {
    const result = HippoCustomFieldSchema.safeParse({
      label: "Count",
      type: "number",
      value: 5,
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.options).toBeUndefined();
    }
  });
});
