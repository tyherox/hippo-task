import { z } from "zod";
import { isDateOnOrBefore } from "../utils/dates.js";
import { SchemaVersionSchema } from "../versioning/versions.js";
import { HippoCustomFieldSchema } from "./custom-field.js";
import {
  IsoDateOrDateTimeStringSchema,
  IsoDateTimeStringSchema,
} from "./date-strings.js";
import {
  DescriptionFormatSchema,
  EstimateUnitSchema,
  HippoPrioritySchema,
  HippoStatusSchema,
} from "./enums.js";
import { HippoPersonSchema } from "./person.js";

/**
 * HippoTask — the central type representing a single unit of work.
 *
 * Required fields: id, title, status, created_at, updated_at, schema_version.
 * Everything else is optional to keep the bar low for quick tasks and AI agents.
 */
export const HippoTaskSchema = z
  .object({
    // ─── Identity ─────────────────────────────────────────────
    /** Unique identifier. UUIDv7 recommended. */
    id: z.string().min(1, "Task ID must be non-empty"),

    /**
     * Map of external platform IDs.
     * Keys are platform names, values are non-empty platform-specific IDs.
     */
    external_ids: z
      .record(
        z.string(),
        z.string().min(1, "External ID value must be non-empty"),
      )
      .optional(),

    // ─── Content ──────────────────────────────────────────────
    /** Task title / summary. Must be non-empty after trimming. */
    title: z
      .string()
      .min(1, "Title must be non-empty")
      .refine((s) => s.trim().length > 0, "Title must not be whitespace-only"),

    /** Task description body. Canonical format is Markdown. */
    description: z.string().optional(),

    /** Hint about the description's format. Defaults to "markdown". */
    description_format: DescriptionFormatSchema.optional(),

    // ─── Workflow ─────────────────────────────────────────────
    /** Normalized status. */
    status: HippoStatusSchema,

    /** Original platform status string, preserved for lossless round-tripping. */
    status_raw: z.string().optional(),

    /** Normalized priority level. */
    priority: HippoPrioritySchema.optional(),

    /** Original platform priority string or number. */
    priority_raw: z.union([z.string(), z.number()]).optional(),

    // ─── People ───────────────────────────────────────────────
    /** Users assigned to this task. */
    assignees: z.array(HippoPersonSchema).optional(),

    /** The user who created this task. */
    creator: HippoPersonSchema.optional(),

    // ─── Time ─────────────────────────────────────────────────
    /** When the task was created. ISO 8601 datetime string. */
    created_at: IsoDateTimeStringSchema,

    /** When the task was last updated. ISO 8601 datetime string. */
    updated_at: IsoDateTimeStringSchema,

    /** When the task is due. ISO 8601 date or datetime string. */
    due_date: IsoDateOrDateTimeStringSchema.optional(),

    /** When work on the task should start. ISO 8601 date or datetime string. */
    start_date: IsoDateOrDateTimeStringSchema.optional(),

    /** When the task was completed/resolved. ISO 8601 datetime string. */
    completed_at: IsoDateTimeStringSchema.optional(),

    // ─── Organization ─────────────────────────────────────────
    /** The project/container this task belongs to. References HippoProject.id. */
    project_id: z.string().optional(),

    /** Parent task ID, for subtask relationships. References another HippoTask.id. */
    parent_id: z.string().optional(),

    /** Free-form labels/tags for categorization. */
    labels: z.array(z.string()).optional(),

    // ─── Estimation ───────────────────────────────────────────
    /** Effort estimate as a positive numeric value. */
    estimate: z.number().positive("Estimate must be greater than 0").optional(),

    /** Unit for the estimate value. Defaults to "points" if omitted. */
    estimate_unit: EstimateUnitSchema.optional(),

    // ─── Extension ────────────────────────────────────────────
    /** Typed custom fields. Keys are field names/slugs. */
    custom_fields: z.record(z.string(), HippoCustomFieldSchema).optional(),

    /** Untyped metadata overflow. Namespaced keys recommended. */
    metadata: z.record(z.string(), z.unknown()).optional(),

    // ─── Schema ───────────────────────────────────────────────
    /**
     * Schema version this task conforms to.
     *
     * Must be one of the versions in the registry
     * ({@link ../versioning/registry.ts}). To accept older payloads
     * and upgrade them to the current shape, use `migrateTask`.
     */
    schema_version: SchemaVersionSchema,
  })
  .refine(
    (task) => {
      if (task.start_date != null && task.due_date != null) {
        // Instant-based comparison — handles mixed date/datetime formats
        // and timezone offsets. Date-only values span their whole UTC day.
        return isDateOnOrBefore(task.start_date, task.due_date);
      }
      return true;
    },
    {
      message: "start_date must be on or before due_date",
      path: ["start_date"],
    },
  );

export type HippoTask = z.infer<typeof HippoTaskSchema>;
