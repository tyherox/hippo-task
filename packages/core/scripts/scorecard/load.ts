/**
 * YAML loading + schema validation for provider scorecard files.
 *
 * Reads every file in docs/scorecard/providers/*.yaml, parses + Zod-validates,
 * and returns the provider records. Both generate.ts and validate.ts use this.
 */

import { readFileSync, readdirSync } from "node:fs";
import { basename, dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import yaml from "js-yaml";
import type { z } from "zod";
import { type ProviderYAML, ProviderYAMLSchema } from "./schema.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

export const PROVIDERS_DIR = resolve(
  __dirname,
  "..",
  "..",
  "..",
  "..",
  "docs",
  "scorecard",
  "providers",
);

export const DOCS_DIR = resolve(__dirname, "..", "..", "..", "..", "docs");

export interface LoadedProvider {
  readonly file: string;
  readonly yaml: ProviderYAML;
}

export class ProviderLoadError extends Error {
  constructor(
    public readonly file: string,
    public readonly issues: z.ZodIssue[],
  ) {
    const detail = issues
      .map((i) => `  - ${i.path.join(".") || "<root>"}: ${i.message}`)
      .join("\n");
    super(`${file}: schema validation failed\n${detail}`);
    this.name = "ProviderLoadError";
  }
}

export function loadProviders(dir: string = PROVIDERS_DIR): LoadedProvider[] {
  const entries = readdirSync(dir)
    .filter((f) => extname(f) === ".yaml")
    .sort();

  const results: LoadedProvider[] = [];
  for (const entry of entries) {
    const file = join(dir, entry);
    const raw = readFileSync(file, "utf8");
    const parsed: unknown = yaml.load(raw);
    const result = ProviderYAMLSchema.safeParse(parsed);
    if (!result.success) {
      throw new ProviderLoadError(entry, result.error.issues);
    }
    const expectedSlug = basename(entry, ".yaml");
    if (result.data.slug !== expectedSlug) {
      throw new ProviderLoadError(entry, [
        {
          code: "custom",
          path: ["slug"],
          message: `slug "${result.data.slug}" does not match filename "${expectedSlug}.yaml"`,
        },
      ]);
    }
    results.push({ file: entry, yaml: result.data });
  }
  return results;
}
