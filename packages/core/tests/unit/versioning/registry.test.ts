import { describe, expect, it } from "vitest";
import { REGISTRY } from "../../../src/versioning/registry.js";

describe("REGISTRY", () => {
  it("is non-empty", () => {
    expect(REGISTRY.length).toBeGreaterThan(0);
  });

  it("has unique versions", () => {
    const versions = REGISTRY.map((e) => e.version);
    expect(new Set(versions).size).toBe(versions.length);
  });

  it("only allows the first entry to omit migrateFromPrevious", () => {
    for (let i = 1; i < REGISTRY.length; i++) {
      expect(typeof REGISTRY[i]?.migrateFromPrevious).toBe("function");
    }
  });

  it("keeps the first entry with no migrator (it's the origin)", () => {
    expect(REGISTRY[0]?.migrateFromPrevious).toBeUndefined();
  });
});
