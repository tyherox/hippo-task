import { z } from "zod";
import { IsoDateOrDateTimeStringSchema } from "./date-strings.js";
import { HippoPersonSchema } from "./person.js";

/**
 * Fields shared by every custom field variant.
 */
const customFieldBase = {
  /** Human-readable field label. Must be non-empty. */
  label: z.string().min(1, "Custom field label must be non-empty"),

  /**
   * For "select" and "multi_select" types: the allowed options.
   * Optional — may not be known outside the source platform.
   */
  options: z.array(z.string()).optional(),
};

/**
 * HippoCustomField — typed extension field for structured data
 * that doesn't fit core schema fields.
 *
 * Discriminated on `type`: the `value` must match the declared type
 * (docs/SCHEMA.md §10), so adapters can trust the tag when writing
 * back to a platform.
 *
 * Keep the variants in sync with CUSTOM_FIELD_TYPES in
 * {@link ./enums.ts} — the enum is the public closed set, this union
 * is its runtime enforcement.
 */
export const HippoCustomFieldSchema = z.discriminatedUnion("type", [
  z.object({
    ...customFieldBase,
    type: z.literal("string"),
    value: z.string(),
  }),
  z.object({
    ...customFieldBase,
    type: z.literal("number"),
    value: z.number(),
  }),
  z.object({
    ...customFieldBase,
    type: z.literal("boolean"),
    value: z.boolean(),
  }),
  /** ISO 8601 date or datetime string. */
  z.object({
    ...customFieldBase,
    type: z.literal("date"),
    value: IsoDateOrDateTimeStringSchema,
  }),
  /** Single choice. When `options` is known, value should be one of them. */
  z.object({
    ...customFieldBase,
    type: z.literal("select"),
    value: z.string(),
  }),
  /** Multiple choices. When `options` is known, values should be among them. */
  z.object({
    ...customFieldBase,
    type: z.literal("multi_select"),
    value: z.array(z.string()),
  }),
  z.object({
    ...customFieldBase,
    type: z.literal("url"),
    value: z.string().min(1, "URL value must be non-empty"),
  }),
  z.object({
    ...customFieldBase,
    type: z.literal("email"),
    value: z.string().min(1, "Email value must be non-empty"),
  }),
  /** References a HippoPerson. */
  z.object({
    ...customFieldBase,
    type: z.literal("person"),
    value: HippoPersonSchema,
  }),
]);

export type HippoCustomField = z.infer<typeof HippoCustomFieldSchema>;
