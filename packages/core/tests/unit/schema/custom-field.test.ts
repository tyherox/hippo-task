import { describe, expect, it } from "vitest";
import { HippoCustomFieldSchema } from "../../../src/schema/custom-field.js";
import {
  CUSTOM_FIELD_TYPES,
  type CustomFieldType,
} from "../../../src/schema/enums.js";

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
      expect(result.data.options).toEqual([
        "development",
        "staging",
        "production",
      ]);
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

  describe("value must match declared type", () => {
    it("rejects a string value on a number field", () => {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Story Points",
        type: "number",
        value: "banana",
      });
      expect(result.success).toBe(false);
    });

    it("rejects a number value on a string field", () => {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Sprint",
        type: "string",
        value: 42,
      });
      expect(result.success).toBe(false);
    });

    it("rejects a non-boolean value on a boolean field", () => {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Is Blocked",
        type: "boolean",
        value: "yes",
      });
      expect(result.success).toBe(false);
    });

    it("rejects a non-ISO value on a date field", () => {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Deadline",
        type: "date",
        value: "next friday",
      });
      expect(result.success).toBe(false);
    });

    it("accepts an ISO date value on a date field", () => {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Deadline",
        type: "date",
        value: "2026-03-15",
      });
      expect(result.success).toBe(true);
    });

    it("accepts a string array on a multi_select field", () => {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Regions",
        type: "multi_select",
        value: ["us", "eu"],
        options: ["us", "eu", "apac"],
      });
      expect(result.success).toBe(true);
    });

    it("rejects a bare string on a multi_select field", () => {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Regions",
        type: "multi_select",
        value: "us",
      });
      expect(result.success).toBe(false);
    });

    it("rejects a person value without any identifier", () => {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Reviewed By",
        type: "person",
        value: {},
      });
      expect(result.success).toBe(false);
    });
  });

  it("covers every declared custom field type", () => {
    // Compile-time: Record<CustomFieldType, …> fails to build if the enum
    // gains a value without a sample here. Runtime: every enum value must
    // have a parsing variant in the discriminated union.
    const samples: Record<CustomFieldType, unknown> = {
      string: "x",
      number: 1,
      boolean: true,
      date: "2026-03-08",
      select: "a",
      multi_select: ["a"],
      url: "https://example.com",
      email: "a@b.co",
      person: { name: "Jane" },
    };
    for (const type of CUSTOM_FIELD_TYPES) {
      const result = HippoCustomFieldSchema.safeParse({
        label: "Label",
        type,
        value: samples[type],
      });
      expect(result.success, `type "${type}" should have a valid variant`).toBe(
        true,
      );
    }
  });
});
