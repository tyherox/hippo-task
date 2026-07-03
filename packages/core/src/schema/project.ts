import { z } from "zod";
import { SchemaVersionSchema } from "../versioning/versions.js";
import { IsoDateTimeStringSchema } from "./date-strings.js";
import { ProjectStatusSchema } from "./enums.js";

/**
 * HippoProject — groups tasks together.
 *
 * Maps to Jira Project, Asana Project, Linear Project, ClickUp List,
 * Trello Board, Monday Board, Notion Database, GitHub Repo/Project,
 * Todoist Project, Planner Plan.
 */
export const HippoProjectSchema = z.object({
  /** Unique identifier. UUIDv7 recommended. */
  id: z.string().min(1, "Project ID must be non-empty"),

  /** External platform IDs. */
  external_ids: z
    .record(
      z.string().min(1, "External ID keys must be non-empty"),
      z.string().min(1, "External ID value must be non-empty"),
    )
    .optional(),

  /** Project name. Must be non-empty after trimming. */
  name: z
    .string()
    .min(1, "Project name must be non-empty")
    .refine(
      (s) => s.trim().length > 0,
      "Project name must not be whitespace-only",
    ),

  /** Project description. Markdown format. */
  description: z.string().optional(),

  /** Project lifecycle status. */
  status: ProjectStatusSchema.optional(),

  /** ISO 8601 datetime. */
  created_at: IsoDateTimeStringSchema,

  /** ISO 8601 datetime. */
  updated_at: IsoDateTimeStringSchema,

  /** Untyped metadata overflow. */
  metadata: z
    .record(z.string().min(1, "Metadata keys must be non-empty"), z.unknown())
    .optional(),

  /**
   * Schema version this project conforms to.
   *
   * Must be one of the versions in the registry
   * ({@link ../versioning/registry.ts}). Use `migrateProject` to
   * upgrade older payloads.
   */
  schema_version: SchemaVersionSchema,
});

export type HippoProject = z.infer<typeof HippoProjectSchema>;
