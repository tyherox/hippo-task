/**
 * Deep merge two plain objects.
 *
 * - Top-level and nested objects are recursively merged.
 * - Arrays are replaced (not concatenated).
 * - undefined values in source are skipped (target value kept).
 * - null values in source overwrite the target.
 * - Does NOT mutate the target; returns a new object.
 */
export function deepMerge<T extends Record<string, unknown>>(
  target: T,
  source: Partial<T>,
): T {
  const result = { ...target };

  for (const key of Object.keys(source) as Array<keyof T>) {
    const sourceVal = source[key];

    // Skip undefined — means "don't change this field"
    if (sourceVal === undefined) {
      continue;
    }

    // If both are plain objects (not arrays, not null), recurse
    if (
      isPlainObject(sourceVal) &&
      isPlainObject(result[key])
    ) {
      result[key] = deepMerge(
        result[key] as Record<string, unknown>,
        sourceVal as Record<string, unknown>,
      ) as T[keyof T];
    } else {
      result[key] = sourceVal as T[keyof T];
    }
  }

  return result;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.getPrototypeOf(value) === Object.prototype
  );
}
