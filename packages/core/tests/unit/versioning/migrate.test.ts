import { describe, expect, it } from "vitest";
import { HippoTaskSchema } from "../../../src/schema/task.js";
import {
  UnsupportedSchemaVersionError,
  migrateProject,
  migrateTask,
} from "../../../src/versioning/migrate.js";
import { CURRENT_SCHEMA_VERSION } from "../../../src/versioning/versions.js";

const VALID_CURRENT_TASK = {
  id: "019470e1-3a4c-7f1c-8a5e-2b3c4d5e6f7a",
  title: "Migrate me",
  status: "todo",
  created_at: "2026-04-19T10:00:00Z",
  updated_at: "2026-04-19T10:00:00Z",
  schema_version: CURRENT_SCHEMA_VERSION,
};

const VALID_CURRENT_PROJECT = {
  id: "019470e1-3a4c-7f1c-8a5e-1111111111aa",
  name: "Auth Rewrite",
  created_at: "2026-04-01T09:00:00Z",
  updated_at: "2026-04-19T14:30:00Z",
  schema_version: CURRENT_SCHEMA_VERSION,
};

describe("migrateTask", () => {
  it("is identity for current-version payloads", () => {
    const migrated = migrateTask(VALID_CURRENT_TASK);
    expect(migrated).toEqual(HippoTaskSchema.parse(VALID_CURRENT_TASK));
  });

  it("throws UnsupportedSchemaVersionError for an unknown version", () => {
    expect(() =>
      migrateTask({ ...VALID_CURRENT_TASK, schema_version: "0.5.0" }),
    ).toThrow(UnsupportedSchemaVersionError);
  });

  it("attaches version and supportedVersions metadata to the error", () => {
    try {
      migrateTask({ ...VALID_CURRENT_TASK, schema_version: "99.0.0" });
      expect.fail("should have thrown");
    } catch (err) {
      expect(err).toBeInstanceOf(UnsupportedSchemaVersionError);
      const e = err as UnsupportedSchemaVersionError;
      expect(e.version).toBe("99.0.0");
      expect(e.supportedVersions).toContain(CURRENT_SCHEMA_VERSION);
      expect(e.code).toBe("UNSUPPORTED_SCHEMA_VERSION");
    }
  });

  it("throws for payloads without schema_version", () => {
    const { schema_version: _discard, ...noVersion } = VALID_CURRENT_TASK;
    expect(() => migrateTask(noVersion)).toThrow(UnsupportedSchemaVersionError);
  });

  it("throws for non-object input", () => {
    expect(() => migrateTask("not an object")).toThrow(
      UnsupportedSchemaVersionError,
    );
    expect(() => migrateTask(null)).toThrow(UnsupportedSchemaVersionError);
    expect(() => migrateTask(42)).toThrow(UnsupportedSchemaVersionError);
  });

  it("re-validates after migration — malformed payloads fail with ZodError", () => {
    const corrupt = { ...VALID_CURRENT_TASK, status: "banana" };
    expect(() => migrateTask(corrupt)).toThrow();
  });
});

describe("migrateProject", () => {
  it("is identity for current-version payloads", () => {
    const migrated = migrateProject(VALID_CURRENT_PROJECT);
    expect(migrated.name).toBe("Auth Rewrite");
    expect(migrated.schema_version).toBe(CURRENT_SCHEMA_VERSION);
  });

  it("rejects unknown versions", () => {
    expect(() =>
      migrateProject({ ...VALID_CURRENT_PROJECT, schema_version: "abc" }),
    ).toThrow(UnsupportedSchemaVersionError);
  });
});
