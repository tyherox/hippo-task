import { z } from "zod";

/** True if the value is a string with non-whitespace content. */
function hasText(value: string | undefined): boolean {
  return value !== undefined && value.trim().length > 0;
}

/**
 * HippoPerson — lightweight user/person reference.
 *
 * Every field is optional because a person may only be known by their
 * email, or their platform ID, or their display name — but at least one
 * identifier must be present. An empty person is meaningless to every
 * consumer (assignee matching, cross-platform identity resolution).
 */
export const HippoPersonSchema = z
  .object({
    /** HippoTask internal ID. */
    id: z.string().optional(),

    /** Display name. */
    name: z.string().optional(),

    /** Email address — often the best cross-platform identifier. */
    email: z.string().optional(),

    /**
     * External platform user IDs.
     * Keys are platform names, values are platform-specific IDs.
     */
    external_ids: z.record(z.string(), z.string()).optional(),
  })
  .refine(
    (person) =>
      hasText(person.id) ||
      hasText(person.name) ||
      hasText(person.email) ||
      (person.external_ids !== undefined &&
        Object.keys(person.external_ids).length > 0),
    "Person must include at least one identifier: id, name, email, or external_ids",
  );

export type HippoPerson = z.infer<typeof HippoPersonSchema>;
