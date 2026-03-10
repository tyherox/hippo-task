import type {
  HippoTask,
  HippoStatus,
  HippoPriority,
  DescriptionFormat,
  EstimateUnit,
  HippoPerson,
  HippoCustomField,
} from "@hippotask/core";

/**
 * Input for creating a task on a platform via an adapter.
 * Similar to CreateTaskInput from @hippotask/core but oriented
 * toward what adapters need for platform writes.
 */
export interface HippoTaskCreate {
  title: string;
  description?: string;
  description_format?: DescriptionFormat;
  status?: HippoStatus;
  priority?: HippoPriority;
  assignees?: HippoPerson[];
  due_date?: string;
  start_date?: string;
  project_id?: string;
  parent_id?: string;
  labels?: string[];
  estimate?: number;
  estimate_unit?: EstimateUnit;
  custom_fields?: Record<string, HippoCustomField>;
  metadata?: Record<string, unknown>;
}

/**
 * Input for updating a task on a platform via an adapter.
 * All fields optional — only provided fields are updated.
 */
export type HippoTaskUpdate = Partial<HippoTaskCreate>;
