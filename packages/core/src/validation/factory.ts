import type { HippoCustomField } from "../schema/custom-field.js";
import type { HippoStatus } from "../schema/enums.js";
import type {
  DescriptionFormat,
  EstimateUnit,
  HippoPriority,
} from "../schema/enums.js";
import type { HippoPerson } from "../schema/person.js";
import type { HippoProject } from "../schema/project.js";
import type { HippoTask } from "../schema/task.js";
import { nowISO } from "../utils/dates.js";
import { generateId } from "../utils/id.js";
import { CURRENT_SCHEMA_VERSION } from "../versioning/versions.js";

/** Default status for new tasks. */
const DEFAULT_STATUS: HippoStatus = "todo";

/**
 * Copy of `fields` with every `undefined` entry removed.
 *
 * Preserves the factory invariant: optional fields are absent from the
 * output, never present with an `undefined` value (see AGENTS.md
 * "Optional fields pattern").
 */
function definedFields<T extends Record<string, unknown>>(
  fields: T,
): Partial<T> {
  const out: Partial<T> = {};
  for (const key of Object.keys(fields) as Array<keyof T>) {
    if (fields[key] !== undefined) {
      out[key] = fields[key];
    }
  }
  return out;
}

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
    // Optional fields — omitted entirely when not provided
    ...definedFields({
      external_ids: input.external_ids,
      description: input.description,
      description_format: input.description_format,
      status_raw: input.status_raw,
      priority: input.priority,
      priority_raw: input.priority_raw,
      assignees: input.assignees,
      creator: input.creator,
      due_date: input.due_date,
      start_date: input.start_date,
      project_id: input.project_id,
      parent_id: input.parent_id,
      labels: input.labels,
      estimate: input.estimate,
      estimate_unit: input.estimate_unit,
      custom_fields: input.custom_fields,
      metadata: input.metadata,
    }),
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
    // Optional fields — omitted entirely when not provided
    ...definedFields({
      description: input.description,
      status: input.status,
      external_ids: input.external_ids,
      metadata: input.metadata,
    }),
  };
}
