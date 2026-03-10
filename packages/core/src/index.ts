// ─── @hippotask/core ────────────────────────────────────────────
// Universal task schema, validation, and utilities for HippoTask.
//
// Public API — curated exports only. Internal modules are not exported.
// ─────────────────────────────────────────────────────────────────

// ─── Schemas (Zod) ──────────────────────────────────────────────
export { HippoTaskSchema } from "./schema/task.js";
export { HippoProjectSchema } from "./schema/project.js";
export { HippoPersonSchema } from "./schema/person.js";
export { HippoCustomFieldSchema } from "./schema/custom-field.js";
export { TaskQuerySchema, PaginatedResultSchema } from "./schema/query.js";
export { TaskChangeEventSchema } from "./schema/events.js";
export {
  HippoStatusSchema,
  HippoPrioritySchema,
  DescriptionFormatSchema,
  EstimateUnitSchema,
  ProjectStatusSchema,
  CustomFieldTypeSchema,
  HIPPO_STATUSES,
  HIPPO_PRIORITIES,
  DESCRIPTION_FORMATS,
  ESTIMATE_UNITS,
  PROJECT_STATUSES,
  CUSTOM_FIELD_TYPES,
} from "./schema/enums.js";

// ─── Types (inferred from Zod) ──────────────────────────────────
export type { HippoTask } from "./schema/task.js";
export type { HippoProject } from "./schema/project.js";
export type { HippoPerson } from "./schema/person.js";
export type { HippoCustomField } from "./schema/custom-field.js";
export type { TaskQuery, PaginatedResult } from "./schema/query.js";
export type { TaskChangeEvent } from "./schema/events.js";
export type {
  HippoStatus,
  HippoPriority,
  DescriptionFormat,
  EstimateUnit,
  ProjectStatus,
  CustomFieldType,
} from "./schema/enums.js";

// ─── Validation ─────────────────────────────────────────────────
export { validateTask, validateProject } from "./validation/validate.js";

// ─── Factory Functions ──────────────────────────────────────────
export { createTask, createProject } from "./validation/factory.js";
export type { CreateTaskInput, CreateProjectInput } from "./validation/factory.js";

// ─── Utilities ──────────────────────────────────────────────────
export { generateId } from "./utils/id.js";
export { nowISO, isValidISO8601, isDateOnOrBefore } from "./utils/dates.js";
export { deepMerge } from "./utils/merge.js";

// ─── Errors ─────────────────────────────────────────────────────
export { HippoError, ValidationError } from "./errors.js";
