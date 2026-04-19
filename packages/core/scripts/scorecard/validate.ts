/**
 * Validate provider scorecard YAMLs and the artifacts they produce.
 *
 * Checks:
 *   1. Every docs/scorecard/providers/*.yaml parses + passes the Zod schema
 *      (filename must match slug; sub_criteria IDs must appear in the
 *      exact rubric order).
 *   2. Stated band for D5 isn't higher than the numeric band implied by
 *      event_count (catches "claimed 5, documented 3 events" mistakes).
 *   3. Every sub-criterion marked `met: true` has an `evidence` URL.
 *   4. Every provider has a non-empty `bottom_line` and `gotcha`.
 *   5. All providers share the same verified_at date (or the generator
 *      will reject).
 *   6. docs/scorecard.json on disk matches what the current YAMLs would
 *      regenerate (no drift between source-of-truth and artifact).
 *   7. docs/PROVIDER_SCORECARD.md autogen sections match fresh render.
 *
 * Exits non-zero on any failure. Run: pnpm scorecard:validate
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { computeScores } from "./compute.js";
import { DOCS_DIR, loadProviders } from "./load.js";
import {
  type ScoredProvider,
  buildScorecardJson,
  rankProviders,
  renderDetailedRatings,
  renderGotchasTable,
  renderReadmeTable,
  renderScorecardTable,
} from "./render.js";
import { bandCeilFromEventCount } from "./schema.js";

const SCORECARD_MD = join(DOCS_DIR, "PROVIDER_SCORECARD.md");
const SCORECARD_JSON = join(DOCS_DIR, "scorecard.json");
const HISTORY_JSON = join(DOCS_DIR, "scorecard-history.json");
const README_MD = join(DOCS_DIR, "..", "README.md");

interface Failure {
  readonly file: string;
  readonly path: string;
  readonly message: string;
}

function validateEvidenceForMetCriteria(
  scored: readonly ScoredProvider[],
): Failure[] {
  const failures: Failure[] = [];
  for (const p of scored) {
    const pushIfMissing = (
      sectionKey: string,
      subCriteria: readonly { id: string; met: boolean; evidence?: string }[],
    ): void => {
      for (const c of subCriteria) {
        if (c.met && !c.evidence) {
          failures.push({
            file: `${p.yaml.slug}.yaml`,
            path: `scores.${sectionKey}.sub_criteria.${c.id}.evidence`,
            message: `sub-criterion "${c.id}" is marked met=true but has no evidence URL`,
          });
        }
      }
    };
    pushIfMissing(
      "api_completeness",
      p.yaml.scores.api_completeness.sub_criteria,
    );
    pushIfMissing("api_ergonomics", p.yaml.scores.api_ergonomics.sub_criteria);
    pushIfMissing(
      "auth_simplicity",
      p.yaml.scores.auth_simplicity.sub_criteria,
    );
    pushIfMissing(
      "ai_agent_readiness",
      p.yaml.scores.ai_agent_readiness.sub_criteria,
    );
  }
  return failures;
}

function validateD5ConsistentWithEventCount(
  scored: readonly ScoredProvider[],
): Failure[] {
  const failures: Failure[] = [];
  for (const p of scored) {
    const rt = p.yaml.scores.real_time;
    // Bands 1 and 2 are qualitative (no push / delta-polling only / add-on-
    // or board-scoped webhooks) — event_count doesn't govern them. For
    // bands 3+ the rubric ties the band directly to webhook breadth, so
    // a claimed band above the ceiling implied by event_count is a real
    // inconsistency.
    if (rt.band <= 2) continue;
    const ceil = bandCeilFromEventCount(rt.event_count);
    if (rt.band > ceil) {
      failures.push({
        file: `${p.yaml.slug}.yaml`,
        path: "scores.real_time",
        message: `band=${rt.band} exceeds the ceiling (${ceil}) implied by event_count=${rt.event_count}`,
      });
    }
  }
  return failures;
}

function validateVerifiedAt(scored: readonly ScoredProvider[]): Failure[] {
  const dates = new Set(scored.map((p) => p.yaml.verified_at));
  if (dates.size <= 1) return [];
  return [
    {
      file: "(all)",
      path: "verified_at",
      message: `providers have mixed verified_at dates: ${[...dates].sort().join(", ")}`,
    },
  ];
}

function validateArtifactsInSync(
  scored: readonly ScoredProvider[],
  ranked: readonly ScoredProvider[],
): Failure[] {
  const failures: Failure[] = [];

  // scorecard.json
  const latest = loadLatestHistoryVersions();
  const verifiedAt = ranked[0]?.yaml.verified_at;
  if (!verifiedAt) return [];
  const regenJson = buildScorecardJson(ranked, {
    verified_at: verifiedAt,
    scorecard_version: latest.scorecard_version,
    rubric_version: latest.rubric_version,
  });
  const onDiskJson = JSON.parse(
    readFileSync(SCORECARD_JSON, "utf8"),
  ) as unknown;
  if (JSON.stringify(regenJson) !== JSON.stringify(onDiskJson)) {
    failures.push({
      file: "scorecard.json",
      path: "(file)",
      message:
        "docs/scorecard.json is out of sync with docs/scorecard/providers/*.yaml — run `pnpm scorecard:generate`",
    });
  }

  // PROVIDER_SCORECARD.md autogen sections
  const md = readFileSync(SCORECARD_MD, "utf8");
  const scorecardChecks: Array<{ marker: string; expected: string }> = [
    {
      marker: "AUTOGEN_VERIFIED_AT",
      expected: `*Verified: **${verifiedAt}** — rubric v${latest.rubric_version}, scorecard v${latest.scorecard_version}*`,
    },
    {
      marker: "AUTOGEN_SCORECARD_TABLE",
      expected: renderScorecardTable(ranked),
    },
    {
      marker: "AUTOGEN_DETAILED_RATINGS",
      expected: renderDetailedRatings(ranked),
    },
    { marker: "AUTOGEN_GOTCHAS", expected: renderGotchasTable(ranked) },
  ];
  for (const c of scorecardChecks) {
    const extracted = extractMarker(md, c.marker);
    if (extracted === null) {
      failures.push({
        file: "PROVIDER_SCORECARD.md",
        path: c.marker,
        message: `missing <!-- BEGIN:${c.marker} --> / <!-- END:${c.marker} --> markers`,
      });
      continue;
    }
    if (extracted.trim() !== c.expected.trim()) {
      failures.push({
        file: "PROVIDER_SCORECARD.md",
        path: c.marker,
        message:
          "section is out of sync with provider YAMLs — run `pnpm scorecard:generate`",
      });
    }
  }

  // README.md autogen sections
  const readme = readFileSync(README_MD, "utf8");
  const readmeChecks: Array<{ marker: string; expected: string }> = [
    {
      marker: "AUTOGEN_README_SCORECARD",
      expected: renderReadmeTable(ranked),
    },
  ];
  for (const c of readmeChecks) {
    const extracted = extractMarker(readme, c.marker);
    if (extracted === null) {
      failures.push({
        file: "README.md",
        path: c.marker,
        message: `missing <!-- BEGIN:${c.marker} --> / <!-- END:${c.marker} --> markers`,
      });
      continue;
    }
    if (extracted.trim() !== c.expected.trim()) {
      failures.push({
        file: "README.md",
        path: c.marker,
        message:
          "section is out of sync with provider YAMLs — run `pnpm scorecard:generate`",
      });
    }
  }
  // keep for unused warning
  void scored;
  return failures;
}

function extractMarker(markdown: string, name: string): string | null {
  const begin = `<!-- BEGIN:${name} -->`;
  const end = `<!-- END:${name} -->`;
  const i = markdown.indexOf(begin);
  if (i < 0) return null;
  const j = markdown.indexOf(end, i + begin.length);
  if (j < 0) return null;
  return markdown.slice(i + begin.length, j).trim();
}

function loadLatestHistoryVersions(): {
  scorecard_version: string;
  rubric_version: string;
} {
  const parsed = JSON.parse(readFileSync(HISTORY_JSON, "utf8")) as {
    history: Array<{ scorecard_version?: string; rubric_version?: string }>;
  };
  const latest = parsed.history.at(-1);
  if (
    !latest ||
    typeof latest.scorecard_version !== "string" ||
    typeof latest.rubric_version !== "string"
  ) {
    throw new Error("scorecard-history.json: last entry missing versions");
  }
  return {
    scorecard_version: latest.scorecard_version,
    rubric_version: latest.rubric_version,
  };
}

function main(): void {
  const providers = loadProviders();
  const scored: ScoredProvider[] = providers.map((p) => ({
    yaml: p.yaml,
    scores: computeScores(p.yaml),
  }));
  const ranked = rankProviders(scored);

  const failures: Failure[] = [
    ...validateEvidenceForMetCriteria(scored),
    ...validateD5ConsistentWithEventCount(scored),
    ...validateVerifiedAt(scored),
    ...validateArtifactsInSync(scored, ranked),
  ];

  if (failures.length > 0) {
    // eslint-disable-next-line no-console
    console.error(
      `\nScorecard validation failed (${failures.length} issue${failures.length === 1 ? "" : "s"}):\n`,
    );
    for (const f of failures) {
      // eslint-disable-next-line no-console
      console.error(`  [${f.file}] ${f.path}: ${f.message}`);
    }
    process.exit(1);
  }

  // eslint-disable-next-line no-console
  console.log(
    `Scorecard validation OK (${providers.length} providers, all artifacts in sync).`,
  );
}

main();
