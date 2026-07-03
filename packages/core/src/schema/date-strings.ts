import { z } from "zod";
import { isValidISO8601, isValidISO8601DateTime } from "../utils/dates.js";

/**
 * ISO 8601 datetime string, e.g. "2026-03-08T10:00:00Z".
 *
 * For timestamp fields where a time component is required
 * (created_at, updated_at, completed_at).
 */
export const IsoDateTimeStringSchema = z
  .string()
  .refine(
    isValidISO8601DateTime,
    'Must be an ISO 8601 datetime string, e.g. "2026-03-08T10:00:00Z"',
  );

/**
 * ISO 8601 date or datetime string, e.g. "2026-03-15" or
 * "2026-03-15T17:00:00Z".
 *
 * For scheduling fields (due_date, start_date) where platforms differ
 * on date vs. datetime precision.
 */
export const IsoDateOrDateTimeStringSchema = z
  .string()
  .refine(
    isValidISO8601,
    'Must be an ISO 8601 date or datetime string, e.g. "2026-03-15" or "2026-03-15T17:00:00Z"',
  );
