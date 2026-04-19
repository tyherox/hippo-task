import { describe, expect, it } from "vitest";
import {
  CURRENT_SCHEMA_VERSION,
  SUPPORTED_VERSIONS,
  SchemaVersionSchema,
  isSupportedVersion,
} from "../../../src/versioning/versions.js";

describe("schema versions", () => {
  it("exposes at least one supported version", () => {
    expect(SUPPORTED_VERSIONS.length).toBeGreaterThan(0);
  });

  it("uses the last supported version as CURRENT_SCHEMA_VERSION", () => {
    expect(CURRENT_SCHEMA_VERSION).toBe(
      SUPPORTED_VERSIONS[SUPPORTED_VERSIONS.length - 1],
    );
  });

  it("currently ships 1.0.0 as the current version", () => {
    expect(CURRENT_SCHEMA_VERSION).toBe("1.0.0");
  });

  describe("SchemaVersionSchema", () => {
    it("accepts every supported version", () => {
      for (const version of SUPPORTED_VERSIONS) {
        const result = SchemaVersionSchema.safeParse(version);
        expect(result.success).toBe(true);
      }
    });

    it("rejects unknown version strings with a helpful message", () => {
      const result = SchemaVersionSchema.safeParse("0.9.0");
      expect(result.success).toBe(false);
      if (result.success) return;
      expect(result.error.issues[0]?.code).toBe("invalid_enum_value");
    });

    it("rejects non-string input", () => {
      expect(SchemaVersionSchema.safeParse(100).success).toBe(false);
      expect(SchemaVersionSchema.safeParse(null).success).toBe(false);
      expect(SchemaVersionSchema.safeParse(undefined).success).toBe(false);
    });
  });

  describe("isSupportedVersion", () => {
    it("returns true for supported versions", () => {
      expect(isSupportedVersion("1.0.0")).toBe(true);
    });

    it("returns false for unsupported versions", () => {
      expect(isSupportedVersion("0.9.0")).toBe(false);
      expect(isSupportedVersion("2.0.0")).toBe(false);
      expect(isSupportedVersion("")).toBe(false);
      expect(isSupportedVersion("latest")).toBe(false);
    });
  });
});
