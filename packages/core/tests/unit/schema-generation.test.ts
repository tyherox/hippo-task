import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { buildJsonSchema } from "../../scripts/build-json-schema.js";
import { HippoProjectSchema } from "../../src/schema/project.js";
import { HippoTaskSchema } from "../../src/schema/task.js";

const SCHEMA_DIR = join(
  fileURLToPath(new URL(".", import.meta.url)),
  "..",
  "..",
  "schema",
);

function loadCommitted(filename: string): unknown {
  return JSON.parse(readFileSync(join(SCHEMA_DIR, filename), "utf8"));
}

/**
 * Guards against drift between the Zod sources and the committed
 * JSON Schema exports. If these fail, run:
 *
 *   pnpm --filter @hippotask/core schema:generate
 */
describe("committed JSON Schema files match the Zod sources", () => {
  it("hippo-task.schema.json is up to date", () => {
    expect(loadCommitted("hippo-task.schema.json")).toEqual(
      buildJsonSchema("HippoTask", "hippo-task.schema.json", HippoTaskSchema),
    );
  });

  it("hippo-project.schema.json is up to date", () => {
    expect(loadCommitted("hippo-project.schema.json")).toEqual(
      buildJsonSchema(
        "HippoProject",
        "hippo-project.schema.json",
        HippoProjectSchema,
      ),
    );
  });
});
