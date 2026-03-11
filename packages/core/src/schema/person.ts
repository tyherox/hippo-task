import { z } from "zod";

/**
 * HippoPerson — lightweight user/person reference.
 *
 * All fields are optional because a person may only be known
 * by their email, or their platform ID, or their display name.
 */
export const HippoPersonSchema = z.object({
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
});

export type HippoPerson = z.infer<typeof HippoPersonSchema>;
