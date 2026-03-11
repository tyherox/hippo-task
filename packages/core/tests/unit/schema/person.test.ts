import { describe, it, expect } from "vitest";
import { HippoPersonSchema } from "../../../src/schema/person.js";

describe("HippoPersonSchema", () => {
  it("accepts a full person", () => {
    const result = HippoPersonSchema.safeParse({
      id: "person-123",
      name: "Alice Chen",
      email: "alice@example.com",
      external_ids: { jira: "5a09f", github: "alicechen" },
    });
    expect(result.success).toBe(true);
  });

  it("accepts a minimal person (empty object)", () => {
    const result = HippoPersonSchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("accepts a person with only name", () => {
    const result = HippoPersonSchema.safeParse({ name: "Bob" });
    expect(result.success).toBe(true);
  });

  it("accepts a person with only email", () => {
    const result = HippoPersonSchema.safeParse({ email: "bob@example.com" });
    expect(result.success).toBe(true);
  });

  it("rejects non-string name", () => {
    const result = HippoPersonSchema.safeParse({ name: 42 });
    expect(result.success).toBe(false);
  });

  it("rejects non-string email", () => {
    const result = HippoPersonSchema.safeParse({ email: 42 });
    expect(result.success).toBe(false);
  });

  it("rejects external_ids with non-string values", () => {
    const result = HippoPersonSchema.safeParse({
      external_ids: { jira: 123 },
    });
    expect(result.success).toBe(false);
  });
});
