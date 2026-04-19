import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { HippoProjectSchema } from "../../src/schema/project.js";
import { HippoTaskSchema } from "../../src/schema/task.js";

const FIXTURES_DIR = join(
  fileURLToPath(new URL(".", import.meta.url)),
  "..",
  "fixtures",
);

function load(relPath: string): unknown {
  return JSON.parse(readFileSync(join(FIXTURES_DIR, relPath), "utf8"));
}

function jsonFilesIn(subdir: string): string[] {
  return readdirSync(join(FIXTURES_DIR, subdir))
    .filter((name) => name.endsWith(".json"))
    .sort();
}

describe("fixtures/valid", () => {
  const taskFiles = jsonFilesIn("valid").filter((f) => f.includes("task"));
  const projectFiles = jsonFilesIn("valid").filter((f) =>
    f.includes("project"),
  );

  it.each(taskFiles)("task fixture %s parses cleanly", (name) => {
    const result = HippoTaskSchema.safeParse(load(`valid/${name}`));
    if (!result.success) {
      throw new Error(
        `Expected ${name} to parse, got issues: ${JSON.stringify(result.error.issues, null, 2)}`,
      );
    }
    expect(result.success).toBe(true);
  });

  it.each(projectFiles)("project fixture %s parses cleanly", (name) => {
    const result = HippoProjectSchema.safeParse(load(`valid/${name}`));
    if (!result.success) {
      throw new Error(
        `Expected ${name} to parse, got issues: ${JSON.stringify(result.error.issues, null, 2)}`,
      );
    }
    expect(result.success).toBe(true);
  });
});

describe("fixtures/invalid", () => {
  const invalidTaskFixtures: Record<string, RegExp> = {
    "empty-title.json": /title/i,
    "whitespace-title.json": /title/i,
    "bad-status.json": /status/i,
    "dates-out-of-order.json": /start_date/i,
    "missing-required.json": /schema_version/i,
  };

  for (const [name, pathMatcher] of Object.entries(invalidTaskFixtures)) {
    it(`${name} fails validation with a relevant error`, () => {
      const result = HippoTaskSchema.safeParse(load(`invalid/${name}`));
      expect(result.success).toBe(false);
      if (result.success) return;
      const paths = result.error.issues.map((i) => i.path.join(".")).join(",");
      expect(paths).toMatch(pathMatcher);
    });
  }
});
