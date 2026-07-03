import { describe, expect, it } from "vitest";
import {
  isDateOnOrBefore,
  isValidISO8601,
  isValidISO8601DateTime,
  nowISO,
} from "../../../src/utils/dates.js";

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

describe("isValidISO8601DateTime", () => {
  it("accepts datetime with Z", () => {
    expect(isValidISO8601DateTime("2026-03-08T10:00:00Z")).toBe(true);
  });

  it("accepts datetime with offset", () => {
    expect(isValidISO8601DateTime("2026-03-08T10:00:00+05:30")).toBe(true);
  });

  it("accepts datetime with milliseconds", () => {
    expect(isValidISO8601DateTime("2026-03-08T10:00:00.123Z")).toBe(true);
  });

  it("rejects date-only strings", () => {
    expect(isValidISO8601DateTime("2026-03-08")).toBe(false);
  });

  it("rejects empty string and plain text", () => {
    expect(isValidISO8601DateTime("")).toBe(false);
    expect(isValidISO8601DateTime("yesterday")).toBe(false);
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

  it("treats a datetime as on-or-before a date-only bound naming the same UTC day", () => {
    expect(isDateOnOrBefore("2026-03-08T10:00:00Z", "2026-03-08")).toBe(true);
  });

  it("returns false when a date-only a is after a datetime b on an earlier day", () => {
    expect(isDateOnOrBefore("2026-03-09", "2026-03-08T23:00:00Z")).toBe(false);
  });

  it("compares instants across timezone offsets, not lexicographically", () => {
    // "2026-03-08T23:00:00-05:00" is 2026-03-09T04:00:00Z.
    expect(
      isDateOnOrBefore("2026-03-09T01:00:00Z", "2026-03-08T23:00:00-05:00"),
    ).toBe(true);
    expect(
      isDateOnOrBefore("2026-03-08T23:00:00-05:00", "2026-03-09T01:00:00Z"),
    ).toBe(false);
  });

  it("returns false for unparseable input", () => {
    expect(isDateOnOrBefore("banana", "2026-03-08")).toBe(false);
    expect(isDateOnOrBefore("2026-03-08", "banana")).toBe(false);
  });
});
