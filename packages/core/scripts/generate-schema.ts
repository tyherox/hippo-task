/**
 * Generate JSON Schema files from Zod definitions.
 *
 * Output:
 *   packages/core/schema/hippo-task.schema.json
 *   packages/core/schema/hippo-project.schema.json
 */
import { writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { zodToJsonSchema } from "zod-to-json-schema";
import { HippoTaskSchema } from "../src/schema/task.js";
import { HippoProjectSchema } from "../src/schema/project.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const schemaDir = join(__dirname, "..", "schema");

mkdirSync(schemaDir, { recursive: true });

// Generate HippoTask JSON Schema
const taskJsonSchema = zodToJsonSchema(HippoTaskSchema, {
  name: "HippoTask",
  $refStrategy: "none",
});
writeFileSync(
  join(schemaDir, "hippo-task.schema.json"),
  JSON.stringify(taskJsonSchema, null, 2) + "\n",
);

// Generate HippoProject JSON Schema
const projectJsonSchema = zodToJsonSchema(HippoProjectSchema, {
  name: "HippoProject",
  $refStrategy: "none",
});
writeFileSync(
  join(schemaDir, "hippo-project.schema.json"),
  JSON.stringify(projectJsonSchema, null, 2) + "\n",
);

console.log("✅ Generated JSON Schema files in packages/core/schema/");
