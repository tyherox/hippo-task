import { describe, it, expect } from "vitest";
import { generateId } from "../../../src/utils/id.js";

describe("generateId", () => {
  it("returns a non-empty string", () => {
    const id = generateId();
    expect(typeof id).toBe("string");
    expect(id.length).toBeGreaterThan(0);
  });

  it("returns a valid UUID format", () => {
    const id = generateId();
    // UUID format: 8-4-4-4-12 hex chars
    expect(id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,
    );
  });

  it("generates unique IDs", () => {
    const ids = new Set(Array.from({ length: 100 }, () => generateId()));
    expect(ids.size).toBe(100);
  });

  it("generates time-sortable IDs (UUIDv7) across different milliseconds", async () => {
    const id1 = generateId();
    // Wait 2ms to ensure different timestamp
    await new Promise((resolve) => setTimeout(resolve, 2));
    const id2 = generateId();
    // UUIDv7 IDs are lexicographically sortable by time
    // id2 should be > id1 since it was generated later
    expect(id2 > id1).toBe(true);
  });
});
