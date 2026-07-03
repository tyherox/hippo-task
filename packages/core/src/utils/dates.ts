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

  // Reject impossible calendar dates ("2026-02-30"). V8's Date.parse is
  // lenient and rolls these over instead of returning NaN.
  if (!isRealCalendarDate(value)) return false;

  // Verify the remainder (time component) parses to a valid date
  const parsed = Date.parse(value);
  return !Number.isNaN(parsed);
}

/** True if the leading YYYY-MM-DD names a real calendar date. */
function isRealCalendarDate(value: string): boolean {
  const year = Number(value.slice(0, 4));
  const month = Number(value.slice(5, 7));
  const day = Number(value.slice(8, 10));
  if (month < 1 || month > 12) return false;
  // Day 0 of the next month is the last day of `month`.
  const daysInMonth = new Date(Date.UTC(year, month, 0)).getUTCDate();
  return day >= 1 && day <= daysInMonth;
}

/**
 * Checks if a string is a valid ISO 8601 datetime (date + time component).
 *
 * Accepts:
 * - "2026-03-08T10:00:00Z" (datetime with Z)
 * - "2026-03-08T10:00:00+05:30" (datetime with offset)
 * - "2026-03-08T10:00:00.123Z" (datetime with ms)
 *
 * Rejects date-only strings ("2026-03-08") — use {@link isValidISO8601}
 * for fields that accept either.
 */
export function isValidISO8601DateTime(value: string): boolean {
  return value.includes("T") && isValidISO8601(value);
}

/** Milliseconds in one UTC day. */
const DAY_MS = 86_400_000;

/**
 * Resolve an ISO 8601 date or datetime string to the [start, end] instant
 * range it covers, in epoch milliseconds.
 *
 * Datetime strings cover a single instant. Date-only strings cover their
 * whole UTC day, from 00:00:00.000 to 23:59:59.999.
 */
function instantRange(value: string): readonly [number, number] {
  if (value.includes("T")) {
    const instant = Date.parse(value);
    return [instant, instant];
  }
  const dayStart = Date.parse(`${value}T00:00:00.000Z`);
  return [dayStart, dayStart + DAY_MS - 1];
}

/**
 * Returns true if date string `a` is on or before date string `b`.
 *
 * Both strings should be ISO 8601 date or datetime strings. Values are
 * compared as instants — not lexicographically — so mixed formats and
 * timezone offsets compare correctly. Date-only strings are interpreted
 * inclusively as their whole UTC day: `a` counts from the start of its
 * day and `b` until the end of its day, so a datetime is "on" a
 * date-only bound that names the same UTC day.
 *
 * Returns false if either string cannot be parsed.
 */
export function isDateOnOrBefore(a: string, b: string): boolean {
  return instantRange(a)[0] <= instantRange(b)[1];
}
