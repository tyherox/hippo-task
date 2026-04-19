import { HippoProjectSchema } from "../schema/project.js";
import type { HippoProject } from "../schema/project.js";
import { HippoTaskSchema } from "../schema/task.js";
import type { HippoTask } from "../schema/task.js";
import { REGISTRY, type VersionRegistryEntry } from "./registry.js";
import { SUPPORTED_VERSIONS, isSupportedVersion } from "./versions.js";

/**
 * Error thrown when a document's `schema_version` is not in the registry.
 *
 * Carries the offending version string and the list of versions this
 * build can parse, so the consumer can decide whether to upgrade
 * HippoTask, migrate the document externally, or drop the record.
 */
export class UnsupportedSchemaVersionError extends Error {
  readonly code = "UNSUPPORTED_SCHEMA_VERSION";
  readonly version: string;
  readonly supportedVersions: readonly string[];

  constructor(version: string) {
    super(
      `Schema version "${version}" is not supported by this build. ` +
        `Supported versions: ${SUPPORTED_VERSIONS.join(", ")}.`,
    );
    this.name = "UnsupportedSchemaVersionError";
    this.version = version;
    this.supportedVersions = SUPPORTED_VERSIONS;
  }
}

/**
 * Extract `schema_version` from an unknown payload without parsing the
 * full document. Used by migrate functions to pick the starting point
 * in the registry chain before the payload has been validated.
 */
function extractSchemaVersion(input: unknown): string {
  if (typeof input !== "object" || input === null) {
    throw new UnsupportedSchemaVersionError("<not an object>");
  }
  const value = (input as { schema_version?: unknown }).schema_version;
  if (typeof value !== "string" || value.length === 0) {
    throw new UnsupportedSchemaVersionError("<missing>");
  }
  return value;
}

/**
 * Walk a registry chain from `fromVersion` forward, applying each
 * entry's `migrateFromPrevious`. Returns the upgraded (still unvalidated)
 * payload at the last entry's version.
 *
 * Exported so tests can exercise multi-step chains with a synthetic
 * registry. Public callers should use {@link migrateTask} or
 * {@link migrateProject} — those also run Zod validation after the chain.
 *
 * @param input - The payload to upgrade.
 * @param fromVersion - The payload's current `schema_version`.
 * @param registry - The chain to walk. Defaults to the real
 *   {@link REGISTRY}; tests can pass a synthetic tuple.
 */
export function applyMigrationChain(
  input: unknown,
  fromVersion: string,
  registry: readonly VersionRegistryEntry[] = REGISTRY,
): unknown {
  const startIdx = registry.findIndex((e) => e.version === fromVersion);
  if (startIdx === -1) {
    throw new UnsupportedSchemaVersionError(fromVersion);
  }

  let current: unknown = input;
  for (let i = startIdx + 1; i < registry.length; i++) {
    const entry = registry[i]!;
    if (!entry.migrateFromPrevious) {
      throw new Error(
        `Registry entry ${entry.version} is missing migrateFromPrevious. This is a bug — the registry should have been validated on load.`,
      );
    }
    current = entry.migrateFromPrevious(current);
  }
  return current;
}

/**
 * Upgrade a task payload from any supported schema version to the
 * current version, then validate it.
 *
 * @throws {UnsupportedSchemaVersionError} if the payload's
 *   `schema_version` is not in the registry.
 * @throws {z.ZodError} if the upgraded payload doesn't conform to the
 *   current schema. A migrator bug will surface here, not silently.
 */
export function migrateTask(input: unknown): HippoTask {
  const version = extractSchemaVersion(input);
  if (!isSupportedVersion(version)) {
    throw new UnsupportedSchemaVersionError(version);
  }
  const upgraded = applyMigrationChain(input, version);
  return HippoTaskSchema.parse(upgraded);
}

/** Same as {@link migrateTask} but for HippoProject payloads. */
export function migrateProject(input: unknown): HippoProject {
  const version = extractSchemaVersion(input);
  if (!isSupportedVersion(version)) {
    throw new UnsupportedSchemaVersionError(version);
  }
  const upgraded = applyMigrationChain(input, version);
  return HippoProjectSchema.parse(upgraded);
}
