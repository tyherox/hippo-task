import { describe, it, expect } from "vitest";
import { mapPriority, mapPriorityReverse } from "../../src/priority-map.js";
import type { PriorityMap } from "../../src/priority-map.js";

// Linear-style priority map (numeric, inverted)
const LINEAR_MAP: PriorityMap = {
  toHippo: {
    "0": "none",
    "1": "urgent",
    "2": "high",
    "3": "medium",
    "4": "low",
  },
  fromHippo: {
    none: 0,
    low: 4,
    medium: 3,
    high: 2,
    urgent: 1,
  },
  fallbackPriority: "none",
};

describe("mapPriority", () => {
  it("maps a numeric platform priority to HippoPriority", () => {
    const result = mapPriority(1, LINEAR_MAP);
    expect(result.priority).toBe("urgent");
    expect(result.priority_raw).toBe(1);
  });

  it("maps a string platform priority", () => {
    const result = mapPriority("3", LINEAR_MAP);
    expect(result.priority).toBe("medium");
    expect(result.priority_raw).toBe("3");
  });

  it("uses fallback for unknown priority", () => {
    const result = mapPriority(99, LINEAR_MAP);
    expect(result.priority).toBe("none");
    expect(result.priority_raw).toBe(99);
  });

  it("maps all known priorities correctly", () => {
    expect(mapPriority(0, LINEAR_MAP).priority).toBe("none");
    expect(mapPriority(1, LINEAR_MAP).priority).toBe("urgent");
    expect(mapPriority(2, LINEAR_MAP).priority).toBe("high");
    expect(mapPriority(3, LINEAR_MAP).priority).toBe("medium");
    expect(mapPriority(4, LINEAR_MAP).priority).toBe("low");
  });
});

describe("mapPriorityReverse", () => {
  it("maps HippoPriority to platform priority value", () => {
    expect(mapPriorityReverse("urgent", LINEAR_MAP)).toBe(1);
    expect(mapPriorityReverse("high", LINEAR_MAP)).toBe(2);
    expect(mapPriorityReverse("medium", LINEAR_MAP)).toBe(3);
    expect(mapPriorityReverse("low", LINEAR_MAP)).toBe(4);
    expect(mapPriorityReverse("none", LINEAR_MAP)).toBe(0);
  });
});
