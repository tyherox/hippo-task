import { z } from "zod";
import { REGISTRY_VERSIONS, type RegistryVersion } from "./registry.js";

/**
 * All schema versions this build can parse, as a narrowly-typed tuple.
 *
 * Derived from the version registry — the single source of truth for
 * "what versions exist." Adding an entry to the registry automatically
 * updates this tuple (and every downstream type: `SchemaVersion`,
 * `CURRENT_SCHEMA_VERSION`, `HippoTask["schema_version"]`, etc.).
 */
export const SUPPORTED_VERSIONS = REGISTRY_VERSIONS;

/**
 * The current schema version. New tasks and projects are created with this.
 *
 * Always the last entry in the registry — the chain's terminus. Typed as
 * the literal version string (e.g. `"1.0.0"`), not wide `string`, so
 * consumers get compile-time narrowing.
 */
export const CURRENT_SCHEMA_VERSION = REGISTRY_VERSIONS[
  REGISTRY_VERSIONS.length - 1
] as RegistryVersion;

/**
 * Union of all supported schema version string literals.
 *
 * Grows as the registry grows. Narrower than `string`, so assigning an
 * arbitrary string to `task.schema_version` fails at compile time.
 */
export type SchemaVersion = RegistryVersion;

/**
 * Zod schema for the `schema_version` field.
 *
 * Accepts only versions present in the registry. Unknown versions fail
 * validation with a message that enumerates the supported set. The
 * inferred type is the literal union (e.g. `"1.0.0"`), matching
 * {@link SchemaVersion}.
 */
export const SchemaVersionSchema = z.enum(REGISTRY_VERSIONS);

/** Whether a given string is a version this build can parse. */
export function isSupportedVersion(version: string): version is SchemaVersion {
  return (REGISTRY_VERSIONS as readonly string[]).includes(version);
}
