/**
 * Schema version registry — ordered chain of every schema version this
 * build can parse.
 *
 * ## Adding a new version
 *
 * Append a new entry to the tuple below. The order is load-bearing:
 * migrations walk the chain from an input's version forward to the last
 * entry, so entries must be listed oldest → newest.
 *
 * Each non-initial entry provides `migrateFromPrevious`, a pure function
 * that upgrades a document from the immediately prior version to this
 * version. Migrators compose: to upgrade from v1 to v3, the chain runs
 * `v1 → v2 → v3` by invoking `v2.migrateFromPrevious` then
 * `v3.migrateFromPrevious`.
 *
 * The initial entry has no `migrateFromPrevious` because there is no
 * prior version. All other entries MUST define one.
 *
 * ## Example (hypothetical future bump)
 *
 * ```ts
 * const REGISTRY_TUPLE = [
 *   { version: "1.0.0" },
 *   {
 *     version: "1.1.0",
 *     migrateFromPrevious: (prev) => ({ ...(prev as object), owner: "unknown" }),
 *   },
 * ] as const;
 * ```
 *
 * See {@link ../../SCHEMA_CHANGELOG.md} for the human changelog.
 */

export interface VersionRegistryEntry {
  readonly version: string;
  readonly migrateFromPrevious?: (previous: unknown) => unknown;
}

/**
 * The narrow tuple form — preserves literal types so `SchemaVersion`,
 * `CURRENT_SCHEMA_VERSION`, and the Zod enum all narrow to the exact
 * version strings (e.g. `"1.0.0"`) rather than the wide `string` type.
 */
const REGISTRY_TUPLE = [
  {
    version: "1.0.0",
  },
] as const satisfies readonly VersionRegistryEntry[];

/**
 * Map a tuple of registry entries to the tuple of their version strings,
 * preserving tuple length and literal types.
 */
type VersionTuple<T extends readonly VersionRegistryEntry[]> = {
  -readonly [K in keyof T]: T[K] extends { version: infer V } ? V : never;
};

/**
 * Tuple of every registered version string, narrowly typed.
 *
 * The runtime array is produced by `map`; the `as unknown as` cast
 * carries the narrow type from {@link REGISTRY_TUPLE}. This is safe
 * because `map` preserves length and each element's value is the
 * corresponding entry's `version` literal.
 */
export const REGISTRY_VERSIONS = REGISTRY_TUPLE.map(
  (entry) => entry.version,
) as unknown as VersionTuple<typeof REGISTRY_TUPLE>;

/** Union of all registered version string literals. */
export type RegistryVersion = VersionTuple<typeof REGISTRY_TUPLE>[number];

/**
 * Runtime-shaped view of the registry as an ordinary array of entries.
 * Use this for iteration; use {@link REGISTRY_VERSIONS} or
 * {@link RegistryVersion} when literal types matter.
 */
export const REGISTRY: readonly VersionRegistryEntry[] = REGISTRY_TUPLE;

/**
 * Runtime check that the registry is well-formed:
 * - Non-empty.
 * - Every entry after the first has a `migrateFromPrevious` function.
 * - Versions are unique.
 *
 * Called on module load so misconfiguration surfaces immediately, not
 * lazily during a migration.
 */
function assertRegistryIntegrity(): void {
  if (REGISTRY.length === 0) {
    throw new Error("Schema registry must contain at least one version");
  }

  const seen = new Set<string>();
  for (let i = 0; i < REGISTRY.length; i++) {
    const entry = REGISTRY[i]!;
    if (seen.has(entry.version)) {
      throw new Error(`Duplicate schema version in registry: ${entry.version}`);
    }
    seen.add(entry.version);

    if (i > 0 && typeof entry.migrateFromPrevious !== "function") {
      throw new Error(
        `Registry entry ${entry.version} must define migrateFromPrevious (only the first entry may omit it).`,
      );
    }
  }
}

assertRegistryIntegrity();
