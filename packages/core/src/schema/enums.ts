import { z } from "zod";

// ─── Status ─────────────────────────────────────────────────────

export const HIPPO_STATUSES = [
  "backlog",
  "todo",
  "in_progress",
  "in_review",
  "done",
  "cancelled",
] as const;

export const HippoStatusSchema = z.enum(HIPPO_STATUSES);
export type HippoStatus = z.infer<typeof HippoStatusSchema>;

// ─── Priority ───────────────────────────────────────────────────

export const HIPPO_PRIORITIES = [
  "none",
  "low",
  "medium",
  "high",
  "urgent",
] as const;

export const HippoPrioritySchema = z.enum(HIPPO_PRIORITIES);
export type HippoPriority = z.infer<typeof HippoPrioritySchema>;

// ─── Description Format ─────────────────────────────────────────

export const DESCRIPTION_FORMATS = [
  "markdown",
  "plaintext",
  "html",
] as const;

export const DescriptionFormatSchema = z.enum(DESCRIPTION_FORMATS);
export type DescriptionFormat = z.infer<typeof DescriptionFormatSchema>;

// ─── Estimate Unit ──────────────────────────────────────────────

export const ESTIMATE_UNITS = ["points", "hours", "minutes"] as const;

export const EstimateUnitSchema = z.enum(ESTIMATE_UNITS);
export type EstimateUnit = z.infer<typeof EstimateUnitSchema>;

// ─── Project Status ─────────────────────────────────────────────

export const PROJECT_STATUSES = [
  "active",
  "paused",
  "completed",
  "archived",
] as const;

export const ProjectStatusSchema = z.enum(PROJECT_STATUSES);
export type ProjectStatus = z.infer<typeof ProjectStatusSchema>;

// ─── Custom Field Type ──────────────────────────────────────────

export const CUSTOM_FIELD_TYPES = [
  "string",
  "number",
  "boolean",
  "date",
  "select",
  "multi_select",
  "url",
  "email",
  "person",
] as const;

export const CustomFieldTypeSchema = z.enum(CUSTOM_FIELD_TYPES);
export type CustomFieldType = z.infer<typeof CustomFieldTypeSchema>;
