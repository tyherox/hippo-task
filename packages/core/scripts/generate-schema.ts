/**
 * Generate JSON Schema files from Zod definitions.
 *
 * Output:
 *   packages/core/schema/hippo-task.schema.json
 *   packages/core/schema/hippo-project.schema.json
 *
 * The output embeds the current schema version in `$id` and `title` so
 * downstream consumers can detect which HippoTask version they're
 * looking at without parsing content.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { zodToJsonSchema } from "zod-to-json-schema";
import { HippoProjectSchema } from "../src/schema/project.js";
import { HippoTaskSchema } from "../src/schema/task.js";
import { CURRENT_SCHEMA_VERSION } from "../src/versioning/versions.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const schemaDir = join(__dirname, "..", "schema");

mkdirSync(schemaDir, { recursive: true });

function writeSchema(
  name: string,
  filename: string,
  zodSchema: Parameters<typeof zodToJsonSchema>[0],
): void {
  const rawSchema = zodToJsonSchema(zodSchema, {
    name,
    $refStrategy: "none",
  }) as Record<string, unknown>;

  const withMetadata = {
    $schema: "http://json-schema.org/draft-07/schema#",
    $id: `https://hippotask.dev/schema/${CURRENT_SCHEMA_VERSION}/${filename}`,
    title: `${name} (schema ${CURRENT_SCHEMA_VERSION})`,
    "x-hippotask-schema-version": CURRENT_SCHEMA_VERSION,
    ...rawSchema,
  };

  writeFileSync(
    join(schemaDir, filename),
    `${JSON.stringify(withMetadata, null, 2)}\n`,
  );
}

writeSchema("HippoTask", "hippo-task.schema.json", HippoTaskSchema);
writeSchema("HippoProject", "hippo-project.schema.json", HippoProjectSchema);

console.log(
  `Generated JSON Schema files (version ${CURRENT_SCHEMA_VERSION}) in packages/core/schema/`,
);
