import type { HippoTask } from "../schema/task.js";
import type { HippoProject } from "../schema/project.js";
import type { HippoStatus } from "../schema/enums.js";
import type { HippoPriority, DescriptionFormat, EstimateUnit } from "../schema/enums.js";
import type { HippoPerson } from "../schema/person.js";
import type { HippoCustomField } from "../schema/custom-field.js";
import { generateId } from "../utils/id.js";
import { nowISO } from "../utils/dates.js";

/** Current schema version for new tasks/projects. */
const CURRENT_SCHEMA_VERSION = "1.0.0";

/** Default status for new tasks. */
const DEFAULT_STATUS: HippoStatus = "todo";

/**
 * Input for creating a new HippoTask.
 * Only `title` is required — everything else has smart defaults.
 */
export interface CreateTaskInput {
  id?: string;
  title: string;
  description?: string;
  description_format?: DescriptionFormat;
  status?: HippoStatus;
  status_raw?: string;
  priority?: HippoPriority;
  priority_raw?: string | number;
  assignees?: HippoPerson[];
  creator?: HippoPerson;
  due_date?: string;
  start_date?: string;
  project_id?: string;
  parent_id?: string;
  labels?: string[];
  estimate?: number;
  estimate_unit?: EstimateUnit;
  custom_fields?: Record<string, HippoCustomField>;
  metadata?: Record<string, unknown>;
  external_ids?: Record<string, string>;
}

/**
 * Create a new HippoTask with smart defaults.
 *
 * Auto-generates: `id` (UUIDv7), `status` ("todo"),
 * `created_at` (now), `updated_at` (now), `schema_version` ("1.0.0").
 *
 * @param input - Partial task data. Only `title` is required.
 * @returns A complete HippoTask object.
 *
 * @example
 * ```typescript
 * const task = createTask({ title: "Fix login bug" });
 * // → { id: "019...", title: "Fix login bug", status: "todo", ... }
 * ```
 */
export function createTask(input: CreateTaskInput): HippoTask {
  const now = nowISO();

  return {
    id: input.id ?? generateId(),
    title: input.title,
    status: input.status ?? DEFAULT_STATUS,
    created_at: now,
    updated_at: now,
    schema_version: CURRENT_SCHEMA_VERSION,
    // Optional fields — only included if provided
    ...(input.external_ids !== undefined && { external_ids: input.external_ids }),
    ...(input.description !== undefined && { description: input.description }),
    ...(input.description_format !== undefined && { description_format: input.description_format }),
    ...(input.status_raw !== undefined && { status_raw: input.status_raw }),
    ...(input.priority !== undefined && { priority: input.priority }),
    ...(input.priority_raw !== undefined && { priority_raw: input.priority_raw }),
    ...(input.assignees !== undefined && { assignees: input.assignees }),
    ...(input.creator !== undefined && { creator: input.creator }),
    ...(input.due_date !== undefined && { due_date: input.due_date }),
    ...(input.start_date !== undefined && { start_date: input.start_date }),
    ...(input.project_id !== undefined && { project_id: input.project_id }),
    ...(input.parent_id !== undefined && { parent_id: input.parent_id }),
    ...(input.labels !== undefined && { labels: input.labels }),
    ...(input.estimate !== undefined && { estimate: input.estimate }),
    ...(input.estimate_unit !== undefined && { estimate_unit: input.estimate_unit }),
    ...(input.custom_fields !== undefined && { custom_fields: input.custom_fields }),
    ...(input.metadata !== undefined && { metadata: input.metadata }),
  };
}

/**
 * Input for creating a new HippoProject.
 * Only `name` is required.
 */
export interface CreateProjectInput {
  id?: string;
  name: string;
  description?: string;
  status?: "active" | "paused" | "completed" | "archived";
  external_ids?: Record<string, string>;
  metadata?: Record<string, unknown>;
}

/**
 * Create a new HippoProject with smart defaults.
 */
export function createProject(input: CreateProjectInput): HippoProject {
  const now = nowISO();

  return {
    id: input.id ?? generateId(),
    name: input.name,
    created_at: now,
    updated_at: now,
    schema_version: CURRENT_SCHEMA_VERSION,
    ...(input.description !== undefined && { description: input.description }),
    ...(input.status !== undefined && { status: input.status }),
    ...(input.external_ids !== undefined && { external_ids: input.external_ids }),
    ...(input.metadata !== undefined && { metadata: input.metadata }),
  };
}
