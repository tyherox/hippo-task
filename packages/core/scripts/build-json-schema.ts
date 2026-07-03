/**
 * Pure JSON Schema builder shared by the generation script
 * (`generate-schema.ts`) and the drift test
 * (`tests/unit/schema-generation.test.ts`).
 *
 * Keeping this side-effect-free lets the test verify that the committed
 * `schema/*.json` files match the Zod sources without touching disk.
 */
import { zodToJsonSchema } from "zod-to-json-schema";
import { CURRENT_SCHEMA_VERSION } from "../src/versioning/versions.js";

export function buildJsonSchema(
  name: string,
  filename: string,
  zodSchema: Parameters<typeof zodToJsonSchema>[0],
): Record<string, unknown> {
  const rawSchema = zodToJsonSchema(zodSchema, {
    name,
    $refStrategy: "none",
  }) as Record<string, unknown>;

  return {
    $schema: "http://json-schema.org/draft-07/schema#",
    $id: `https://hippotask.dev/schema/${CURRENT_SCHEMA_VERSION}/${filename}`,
    title: `${name} (schema ${CURRENT_SCHEMA_VERSION})`,
    "x-hippotask-schema-version": CURRENT_SCHEMA_VERSION,
    ...rawSchema,
  };
}
