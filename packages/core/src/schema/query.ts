import { z } from "zod";
import { HippoStatusSchema, HippoPrioritySchema } from "./enums.js";

/**
 * TaskQuery — filter/sort parameters for listing tasks.
 */
export const TaskQuerySchema = z.object({
  /** Filter by project. */
  project_id: z.string().optional(),

  /** Filter by status — single value or array. */
  status: z
    .union([HippoStatusSchema, z.array(HippoStatusSchema)])
    .optional(),

  /** Filter by assignee (email or HippoTask ID). */
  assignee: z.string().optional(),

  /** Filter by label(s). */
  labels: z.array(z.string()).optional(),

  /** Filter by priority. */
  priority: HippoPrioritySchema.optional(),

  /** Tasks updated after this timestamp. ISO 8601. */
  updated_since: z.string().optional(),

  /** Free-text search in title and description. */
  search: z.string().optional(),

  /** Maximum results to return. Default: 50, Max: 200. */
  limit: z.number().int().min(1).max(200).optional(),

  /** Pagination cursor from a previous result. */
  cursor: z.string().optional(),

  /** Sort field. */
  sort_by: z
    .enum(["created_at", "updated_at", "due_date", "priority", "title"])
    .optional(),

  /** Sort direction. */
  sort_direction: z.enum(["asc", "desc"]).optional(),
});

export type TaskQuery = z.infer<typeof TaskQuerySchema>;

/**
 * PaginatedResult — standard pagination wrapper.
 * Uses z.unknown() for items so it can wrap any entity type.
 */
export const PaginatedResultSchema = z.object({
  /** The items in this page. */
  items: z.array(z.unknown()),

  /** Total number of items (if known). */
  total: z.number().int().nonnegative().optional(),

  /** Cursor for the next page (null if no more pages). */
  next_cursor: z.string().nullable().optional(),

  /** Whether there are more pages. */
  has_more: z.boolean(),
});

export type PaginatedResult<T> = {
  items: T[];
  total?: number;
  next_cursor?: string | null;
  has_more: boolean;
};
