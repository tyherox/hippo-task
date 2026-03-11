import { z } from "zod";
import { CustomFieldTypeSchema } from "./enums.js";

/**
 * HippoCustomField — typed extension field for structured data
 * that doesn't fit core schema fields.
 */
export const HippoCustomFieldSchema = z.object({
  /** Human-readable field label. Must be non-empty. */
  label: z.string().min(1, "Custom field label must be non-empty"),

  /** Field data type. */
  type: CustomFieldTypeSchema,

  /** The field value. Type depends on `type` field. */
  value: z.unknown(),

  /**
   * For "select" and "multi_select" types: the allowed options.
   * Optional — may not be known outside the source platform.
   */
  options: z.array(z.string()).optional(),
});

export type HippoCustomField = z.infer<typeof HippoCustomFieldSchema>;
