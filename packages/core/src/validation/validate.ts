import type { SafeParseReturnType } from "zod";
import { HippoTaskSchema } from "../schema/task.js";
import { HippoProjectSchema } from "../schema/project.js";
import type { HippoTask } from "../schema/task.js";
import type { HippoProject } from "../schema/project.js";

/**
 * Validate unknown data against the HippoTask schema.
 *
 * Returns a discriminated union:
 * - `{ success: true, data: HippoTask }` if valid
 * - `{ success: false, error: ZodError }` if invalid
 *
 * @example
 * ```typescript
 * const result = validateTask(data);
 * if (result.success) {
 *   console.log(result.data.title);
 * } else {
 *   console.error(result.error.issues);
 * }
 * ```
 */
export function validateTask(data: unknown): SafeParseReturnType<unknown, HippoTask> {
  return HippoTaskSchema.safeParse(data);
}

/**
 * Validate unknown data against the HippoProject schema.
 */
export function validateProject(data: unknown): SafeParseReturnType<unknown, HippoProject> {
  return HippoProjectSchema.safeParse(data);
}
