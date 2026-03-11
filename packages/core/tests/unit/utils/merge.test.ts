import { describe, it, expect } from "vitest";
import { deepMerge } from "../../../src/utils/merge.js";

describe("deepMerge", () => {
  it("merges top-level properties", () => {
    const target = { a: 1, b: 2 };
    const source = { b: 3, c: 4 };
    const result = deepMerge(target, source);
    expect(result).toEqual({ a: 1, b: 3, c: 4 });
  });

  it("does not mutate the target", () => {
    const target = { a: 1 };
    const source = { b: 2 };
    deepMerge(target, source);
    expect(target).toEqual({ a: 1 });
  });

  it("merges nested objects", () => {
    const target = { outer: { a: 1, b: 2 } };
    const source = { outer: { b: 3, c: 4 } };
    const result = deepMerge(target, source);
    expect(result).toEqual({ outer: { a: 1, b: 3, c: 4 } });
  });

  it("replaces arrays (does not merge them)", () => {
    const target = { tags: ["a", "b"] };
    const source = { tags: ["c"] };
    const result = deepMerge(target, source);
    expect(result).toEqual({ tags: ["c"] });
  });

  it("handles undefined source values by keeping target", () => {
    const target = { a: 1, b: 2 };
    const source = { a: undefined };
    const result = deepMerge(target, source);
    // undefined in source means "don't change"
    expect(result.b).toBe(2);
  });

  it("handles null source values by setting null", () => {
    const target = { a: 1 };
    const source = { a: null };
    const result = deepMerge(target, source);
    expect(result.a).toBeNull();
  });

  it("handles empty source", () => {
    const target = { a: 1, b: 2 };
    const result = deepMerge(target, {});
    expect(result).toEqual({ a: 1, b: 2 });
  });

  it("handles empty target", () => {
    const source = { a: 1 };
    const result = deepMerge({}, source);
    expect(result).toEqual({ a: 1 });
  });
});
