import { describe, it, expect } from "vitest";
import { nowISO, isValidISO8601, isDateOnOrBefore } from "../../../src/utils/dates.js";

describe("nowISO", () => {
  it("returns a valid ISO 8601 string", () => {
    const now = nowISO();
    expect(typeof now).toBe("string");
    // Should end with Z (UTC)
    expect(now.endsWith("Z")).toBe(true);
    // Should be parseable as a Date
    expect(Number.isNaN(Date.parse(now))).toBe(false);
  });

  it("returns current time (within 2 seconds)", () => {
    const before = Date.now();
    const now = nowISO();
    const after = Date.now();
    const parsed = Date.parse(now);
    expect(parsed).toBeGreaterThanOrEqual(before - 1);
    expect(parsed).toBeLessThanOrEqual(after + 1);
  });
});

describe("isValidISO8601", () => {
  it("accepts full datetime with Z", () => {
    expect(isValidISO8601("2026-03-08T10:00:00Z")).toBe(true);
  });

  it("accepts full datetime with offset", () => {
    expect(isValidISO8601("2026-03-08T10:00:00+05:30")).toBe(true);
  });

  it("accepts date-only", () => {
    expect(isValidISO8601("2026-03-08")).toBe(true);
  });

  it("accepts datetime with milliseconds", () => {
    expect(isValidISO8601("2026-03-08T10:00:00.123Z")).toBe(true);
  });

  it("rejects empty string", () => {
    expect(isValidISO8601("")).toBe(false);
  });

  it("rejects plain text", () => {
    expect(isValidISO8601("not a date")).toBe(false);
  });

  it("rejects numeric string", () => {
    expect(isValidISO8601("12345")).toBe(false);
  });
});

describe("isDateOnOrBefore", () => {
  it("returns true when a < b", () => {
    expect(isDateOnOrBefore("2026-03-01", "2026-03-15")).toBe(true);
  });

  it("returns true when a == b", () => {
    expect(isDateOnOrBefore("2026-03-10", "2026-03-10")).toBe(true);
  });

  it("returns false when a > b", () => {
    expect(isDateOnOrBefore("2026-03-20", "2026-03-10")).toBe(false);
  });

  it("handles datetime strings", () => {
    expect(
      isDateOnOrBefore("2026-03-08T10:00:00Z", "2026-03-08T11:00:00Z"),
    ).toBe(true);
  });
});
