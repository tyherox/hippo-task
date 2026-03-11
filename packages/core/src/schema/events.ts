import { z } from "zod";
import { HippoTaskSchema } from "./task.js";

/**
 * TaskChangeEvent — used by adapters to communicate task changes (for sync).
 */
export const TaskChangeEventSchema = z.object({
  /** The type of change. */
  type: z.enum(["created", "updated", "deleted"]),

  /** The platform that originated this change. */
  platform: z.string().min(1, "Platform must be non-empty"),

  /** The external ID of the changed task. */
  external_id: z.string().min(1, "External ID must be non-empty"),

  /** The full task (for created/updated) or undefined (for deleted). */
  task: HippoTaskSchema.optional(),

  /** ISO 8601 timestamp of when the change occurred. */
  timestamp: z.string().min(1, "Timestamp is required"),
});

export type TaskChangeEvent = z.infer<typeof TaskChangeEventSchema>;
