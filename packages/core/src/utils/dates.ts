/**
 * Returns the current time as an ISO 8601 UTC string.
 *
 * Example: "2026-03-08T10:00:00.000Z"
 */
export function nowISO(): string {
  return new Date().toISOString();
}

/**
 * Checks if a string is a valid ISO 8601 date or datetime.
 *
 * Accepts:
 * - "2026-03-08" (date only)
 * - "2026-03-08T10:00:00Z" (datetime with Z)
 * - "2026-03-08T10:00:00+05:30" (datetime with offset)
 * - "2026-03-08T10:00:00.123Z" (datetime with ms)
 *
 * Rejects:
 * - Empty strings, plain text, bare numbers
 */
export function isValidISO8601(value: string): boolean {
  if (value.length === 0) return false;

  // Must match basic ISO 8601 pattern
  const iso8601Pattern =
    /^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})?)?$/;

  if (!iso8601Pattern.test(value)) return false;

  // Verify it parses to a valid date
  const parsed = Date.parse(value);
  return !Number.isNaN(parsed);
}

/**
 * Returns true if date string `a` is on or before date string `b`.
 *
 * Both strings should be ISO 8601 date or datetime strings.
 * Uses lexicographic comparison which works for ISO 8601 format.
 */
export function isDateOnOrBefore(a: string, b: string): boolean {
  return a <= b;
}
