/**
 * Tests for the migration chain walker using synthetic registries.
 *
 * The real {@link REGISTRY} currently has one entry, so it can't
 * exercise multi-step chains or migrator composition. These tests
 * pass synthetic tuples to {@link applyMigrationChain} to validate
 * the walker against the scenarios that will matter once v1.1.0+ exists.
 */
import { describe, expect, it, vi } from "vitest";
import {
  UnsupportedSchemaVersionError,
  applyMigrationChain,
} from "../../../src/versioning/migrate.js";
import type { VersionRegistryEntry } from "../../../src/versioning/registry.js";

/**
 * Shape used by the test inputs and outputs. Narrow (not an index
 * signature) so we can use dot access without TypeScript's
 * `noPropertyAccessFromIndexSignature` rule or Biome's `useLiteralKeys`
 * rule complaining.
 */
interface TestPayload {
  schema_version?: unknown;
  value?: unknown;
  owner?: unknown;
  added?: unknown;
  marker_a?: unknown;
  marker_b?: unknown;
  original?: unknown;
}

describe("applyMigrationChain", () => {
  it("is identity when starting at the last entry (no steps to apply)", () => {
    const registry: readonly VersionRegistryEntry[] = [{ version: "1.0.0" }];
    const input = { schema_version: "1.0.0", value: 42 };
    expect(applyMigrationChain(input, "1.0.0", registry)).toBe(input);
  });

  it("applies a single migrator when the chain has one step", () => {
    const registry: readonly VersionRegistryEntry[] = [
      { version: "1.0.0" },
      {
        version: "1.1.0",
        migrateFromPrevious: (prev) => ({
          ...(prev as object),
          schema_version: "1.1.0",
          owner: "unknown",
        }),
      },
    ];
    const input = { schema_version: "1.0.0", value: 42 };
    const result = applyMigrationChain(input, "1.0.0", registry) as TestPayload;
    expect(result.schema_version).toBe("1.1.0");
    expect(result.owner).toBe("unknown");
    expect(result.value).toBe(42);
  });

  it("composes migrators in order — skipping intermediate versions", () => {
    const steps: string[] = [];
    const registry: readonly VersionRegistryEntry[] = [
      { version: "1.0.0" },
      {
        version: "1.1.0",
        migrateFromPrevious: (prev) => {
          steps.push("1.0.0 -> 1.1.0");
          return { ...(prev as object), schema_version: "1.1.0" };
        },
      },
      {
        version: "2.0.0",
        migrateFromPrevious: (prev) => {
          steps.push("1.1.0 -> 2.0.0");
          return { ...(prev as object), schema_version: "2.0.0" };
        },
      },
    ];
    const input = { schema_version: "1.0.0" };
    const result = applyMigrationChain(input, "1.0.0", registry) as TestPayload;
    expect(steps).toEqual(["1.0.0 -> 1.1.0", "1.1.0 -> 2.0.0"]);
    expect(result.schema_version).toBe("2.0.0");
  });

  it("starts partway through the chain when fromVersion is intermediate", () => {
    const migrator10to11 = vi.fn((prev: unknown): unknown => ({
      ...(prev as object),
      schema_version: "1.1.0",
    }));
    const migrator11to20 = vi.fn((prev: unknown): unknown => ({
      ...(prev as object),
      schema_version: "2.0.0",
    }));
    const registry: readonly VersionRegistryEntry[] = [
      { version: "1.0.0" },
      { version: "1.1.0", migrateFromPrevious: migrator10to11 },
      { version: "2.0.0", migrateFromPrevious: migrator11to20 },
    ];

    applyMigrationChain({ schema_version: "1.1.0" }, "1.1.0", registry);

    expect(migrator10to11).not.toHaveBeenCalled();
    expect(migrator11to20).toHaveBeenCalledTimes(1);
  });

  it("throws UnsupportedSchemaVersionError when fromVersion is not in the registry", () => {
    const registry: readonly VersionRegistryEntry[] = [
      { version: "1.0.0" },
      {
        version: "1.1.0",
        migrateFromPrevious: (prev) => prev,
      },
    ];
    expect(() => applyMigrationChain({}, "1.5.0", registry)).toThrow(
      UnsupportedSchemaVersionError,
    );
  });

  it("passes the output of one migrator as input to the next", () => {
    const registry: readonly VersionRegistryEntry[] = [
      { version: "1.0.0" },
      {
        version: "1.1.0",
        migrateFromPrevious: (prev) => ({
          ...(prev as object),
          marker_a: true,
        }),
      },
      {
        version: "1.2.0",
        migrateFromPrevious: (prev) => {
          const obj = prev as TestPayload;
          if (obj.marker_a !== true) {
            throw new Error("migrator composition broken");
          }
          return { ...obj, marker_b: true };
        },
      },
    ];

    const result = applyMigrationChain(
      { schema_version: "1.0.0" },
      "1.0.0",
      registry,
    ) as TestPayload;
    expect(result.marker_a).toBe(true);
    expect(result.marker_b).toBe(true);
  });

  it("does not mutate the input (migrators return new objects by convention)", () => {
    const registry: readonly VersionRegistryEntry[] = [
      { version: "1.0.0" },
      {
        version: "1.1.0",
        migrateFromPrevious: (prev) => ({ ...(prev as object), added: 1 }),
      },
    ];
    const input: TestPayload = {
      schema_version: "1.0.0",
      original: true,
    };
    applyMigrationChain(input, "1.0.0", registry);
    expect(Object.keys(input).sort()).toEqual(["original", "schema_version"]);
  });

  it("surfaces errors from a migrator unchanged", () => {
    const registry: readonly VersionRegistryEntry[] = [
      { version: "1.0.0" },
      {
        version: "1.1.0",
        migrateFromPrevious: () => {
          throw new Error("migrator blew up");
        },
      },
    ];
    expect(() =>
      applyMigrationChain({ schema_version: "1.0.0" }, "1.0.0", registry),
    ).toThrow("migrator blew up");
  });
});
