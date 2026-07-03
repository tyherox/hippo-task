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
 *
 * The actual schema construction lives in `build-json-schema.ts` so the
 * drift test (`tests/unit/schema-generation.test.ts`) can verify the
 * committed files stay in sync with the Zod sources.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { HippoProjectSchema } from "../src/schema/project.js";
import { HippoTaskSchema } from "../src/schema/task.js";
import { CURRENT_SCHEMA_VERSION } from "../src/versioning/versions.js";
import { buildJsonSchema } from "./build-json-schema.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const schemaDir = join(__dirname, "..", "schema");

mkdirSync(schemaDir, { recursive: true });

function writeSchema(
  name: string,
  filename: string,
  zodSchema: Parameters<typeof buildJsonSchema>[2],
): void {
  writeFileSync(
    join(schemaDir, filename),
    `${JSON.stringify(buildJsonSchema(name, filename, zodSchema), null, 2)}\n`,
  );
}

writeSchema("HippoTask", "hippo-task.schema.json", HippoTaskSchema);
writeSchema("HippoProject", "hippo-project.schema.json", HippoProjectSchema);

console.log(
  `Generated JSON Schema files (version ${CURRENT_SCHEMA_VERSION}) in packages/core/schema/`,
);
