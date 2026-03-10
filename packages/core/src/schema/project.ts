import { z } from "zod";
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
    .record(z.string(), z.string().min(1, "External ID value must be non-empty"))
    .optional(),

  /** Project name. Must be non-empty. */
  name: z.string().min(1, "Project name must be non-empty"),

  /** Project description. Markdown format. */
  description: z.string().optional(),

  /** Project lifecycle status. */
  status: ProjectStatusSchema.optional(),

  /** ISO 8601 datetime. */
  created_at: z.string().min(1, "created_at is required"),

  /** ISO 8601 datetime. */
  updated_at: z.string().min(1, "updated_at is required"),

  /** Untyped metadata overflow. */
  metadata: z.record(z.string(), z.unknown()).optional(),

  /** Schema version this project conforms to. */
  schema_version: z.string().min(1, "schema_version is required"),
});

export type HippoProject = z.infer<typeof HippoProjectSchema>;
