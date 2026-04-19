/**
 * Markdown rendering for provider scorecard autogen sections.
 *
 * Each function produces one bounded chunk that the generator splices
 * into docs/PROVIDER_SCORECARD.md between `<!-- BEGIN:... -->` /
 * `<!-- END:... -->` markers.
 */

import type { ComputedScores } from "./compute.js";
import type { ProviderYAML, SubCriterionT } from "./schema.js";

export interface ScoredProvider {
  readonly yaml: ProviderYAML;
  readonly scores: ComputedScores;
}

const DIMENSION_LABELS = {
  api_completeness: "API Complete",
  api_ergonomics: "API Ergo",
  schema_flexibility: "Schema Flex",
  rate_limits: "Rate Limits",
  real_time: "Real-Time",
  auth_simplicity: "Auth Simple",
  data_portability: "Data Port",
  ai_agent_readiness: "AI Ready",
} as const;

const DIMENSION_FULL_NAMES = {
  api_completeness: "API Completeness",
  api_ergonomics: "API Ergonomics",
  schema_flexibility: "Schema Flexibility",
  rate_limits: "Rate Limits",
  real_time: "Real-Time",
  auth_simplicity: "Auth Simplicity",
  data_portability: "Data Portability",
  ai_agent_readiness: "AI/Agent Readiness",
} as const;

const MEDALS = ["🥇", "🥈", "🥉"] as const;

/**
 * Rank-sort providers by overall desc, name asc. Ties get the same medal
 * by position, not shared-rank (gold > silver > bronze is always 3 items).
 */
export function rankProviders(
  providers: readonly ScoredProvider[],
): ScoredProvider[] {
  return [...providers].sort((a, b) => {
    if (b.scores.overall !== a.scores.overall) {
      return b.scores.overall - a.scores.overall;
    }
    return a.yaml.name.localeCompare(b.yaml.name);
  });
}

/** Produce the §2 comparative table. */
export function renderScorecardTable(
  ranked: readonly ScoredProvider[],
): string {
  const header =
    "| Provider | API Complete | API Ergo | Schema Flex | Rate Limits | Real-Time | Auth Simple | Data Port | AI Ready | **Overall** |";
  const divider = "|----------|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|";
  const rows = ranked.map((p) => {
    const s = p.scores;
    return `| **${p.yaml.name}** | ${s.api_completeness} | ${s.api_ergonomics} | ${s.schema_flexibility} | ${s.rate_limits} | ${s.real_time} | ${s.auth_simplicity} | ${s.data_portability} | ${s.ai_agent_readiness} | **${s.overall.toFixed(1)}** |`;
  });
  return [header, divider, ...rows].join("\n");
}

function renderSubCriterion(c: SubCriterionT): string {
  const symbol = c.met ? "✓" : "✗";
  return `- ${symbol} ${c.note}`;
}

function renderSumDimension(
  key: keyof typeof DIMENSION_FULL_NAMES,
  dimNumber: number,
  score: number,
  subCriteria: readonly SubCriterionT[],
): string {
  const label = DIMENSION_FULL_NAMES[key];
  const bullets = subCriteria.map(renderSubCriterion).join("\n");
  return `**D${dimNumber} — ${label}: ${score}/5**\n${bullets}`;
}

function renderD8(
  score: number,
  subCriteria: readonly SubCriterionT[],
): string {
  const pointsById: Record<string, number> = {
    official_mcp: 2,
    public_ai_api: 1,
    agent_entity_lifecycle: 1,
    production_signals: 1,
  };
  const bullets = subCriteria
    .map((c) => {
      const symbol = c.met ? "✓" : "✗";
      const pts = pointsById[c.id] ?? 1;
      const prefix = c.met ? `(+${pts})` : "";
      const prefixSpaced = prefix ? ` ${prefix}` : "";
      return `- ${symbol}${prefixSpaced} ${c.note}`;
    })
    .join("\n");
  return `**D8 — AI/Agent Readiness: ${score}/5**\n${bullets}`;
}

function renderBandDimension(
  key: "schema_flexibility" | "data_portability",
  dimNumber: number,
  score: number,
  justification: string,
): string {
  const label = DIMENSION_FULL_NAMES[key];
  return `**D${dimNumber} — ${label}: ${score}/5**\n${justification}`;
}

function renderRateLimits(
  score: number,
  formula: string,
  tier: string,
  justification: string,
): string {
  return `**D4 — Rate Limits: ${score}/5**\nTier scored: ${tier}. ${formula}. ${justification}`;
}

function renderRealTime(
  score: number,
  eventCount: number,
  justification: string,
): string {
  return `**D5 — Real-Time: ${score}/5**\n${eventCount} documented event identifiers. ${justification}`;
}

/** Produce the per-provider §3 block. */
export function renderProviderDetail(p: ScoredProvider, rank: number): string {
  const { yaml: y, scores: s } = p;
  const medal = rank < MEDALS.length ? `${MEDALS[rank]} ` : "";
  const header = `### ${medal}${y.name} — Overall: ${s.overall.toFixed(1)} / 5`;

  const blocks = [
    renderSumDimension(
      "api_completeness",
      1,
      s.api_completeness,
      y.scores.api_completeness.sub_criteria,
    ),
    renderSumDimension(
      "api_ergonomics",
      2,
      s.api_ergonomics,
      y.scores.api_ergonomics.sub_criteria,
    ),
    renderBandDimension(
      "schema_flexibility",
      3,
      s.schema_flexibility,
      y.scores.schema_flexibility.justification,
    ),
    renderRateLimits(
      s.rate_limits,
      y.scores.rate_limits.formula,
      y.scores.rate_limits.tier_scored,
      y.scores.rate_limits.justification,
    ),
    renderRealTime(
      s.real_time,
      y.scores.real_time.event_count,
      y.scores.real_time.justification,
    ),
    renderSumDimension(
      "auth_simplicity",
      6,
      s.auth_simplicity,
      y.scores.auth_simplicity.sub_criteria,
    ),
    renderBandDimension(
      "data_portability",
      7,
      s.data_portability,
      y.scores.data_portability.justification,
    ),
    renderD8(s.ai_agent_readiness, y.scores.ai_agent_readiness.sub_criteria),
  ];

  const bottomLine = `**Bottom line:** ${y.bottom_line}`;
  return [header, ...blocks, bottomLine].join("\n\n");
}

/** Produce the §3 detailed-ratings block for all providers, with
 *  horizontal rules between them matching the existing doc style. */
export function renderDetailedRatings(
  ranked: readonly ScoredProvider[],
): string {
  return ranked.map((p, i) => renderProviderDetail(p, i)).join("\n\n---\n\n");
}

/** Produce the §5 gotchas table. */
export function renderGotchasTable(ranked: readonly ScoredProvider[]): string {
  const header = "| Provider | Biggest Gotcha |";
  const divider = "|----------|----------------|";
  const rows = ranked.map((p) => `| **${p.yaml.name}** | ${p.yaml.gotcha} |`);
  return [header, divider, ...rows].join("\n");
}

/** Produce the compact README at-a-glance table (rank → score → best-for). */
export function renderReadmeTable(ranked: readonly ScoredProvider[]): string {
  const header = "| Provider | Overall | Best For |";
  const divider = "|----------|:-------:|----------|";
  const rows = ranked.map(
    (p) =>
      `| **${p.yaml.name}** | ⭐ ${p.scores.overall.toFixed(1)} | ${p.yaml.best_for} |`,
  );
  return [header, divider, ...rows].join("\n");
}

/** Produce the scorecard.json structure for machine consumers. */
export function buildScorecardJson(
  ranked: readonly ScoredProvider[],
  meta: {
    readonly verified_at: string;
    readonly scorecard_version: string;
    readonly rubric_version: string;
  },
): unknown {
  return {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    verified_at: meta.verified_at,
    scorecard_version: meta.scorecard_version,
    rubric_version: meta.rubric_version,
    rubric_url:
      "https://github.com/hippotask/hippo-task/blob/main/docs/SCORECARD_RUBRIC.md",
    providers: ranked.map((p) => ({
      name: p.yaml.name,
      slug: p.yaml.slug,
      overall: p.scores.overall,
      scores: {
        api_completeness: {
          value: p.scores.api_completeness,
          sub_criteria: p.yaml.scores.api_completeness.sub_criteria,
        },
        api_ergonomics: {
          value: p.scores.api_ergonomics,
          sub_criteria: p.yaml.scores.api_ergonomics.sub_criteria,
        },
        schema_flexibility: {
          value: p.scores.schema_flexibility,
          justification: p.yaml.scores.schema_flexibility.justification,
          evidence: p.yaml.scores.schema_flexibility.evidence ?? null,
        },
        rate_limits: {
          value: p.scores.rate_limits,
          effective_ops_per_min:
            p.yaml.scores.rate_limits.effective_ops_per_min,
          tier_scored: p.yaml.scores.rate_limits.tier_scored,
          formula: p.yaml.scores.rate_limits.formula,
          justification: p.yaml.scores.rate_limits.justification,
          evidence: p.yaml.scores.rate_limits.evidence ?? null,
        },
        real_time: {
          value: p.scores.real_time,
          event_count: p.yaml.scores.real_time.event_count,
          justification: p.yaml.scores.real_time.justification,
          evidence: p.yaml.scores.real_time.evidence ?? null,
        },
        auth_simplicity: {
          value: p.scores.auth_simplicity,
          sub_criteria: p.yaml.scores.auth_simplicity.sub_criteria,
        },
        data_portability: {
          value: p.scores.data_portability,
          justification: p.yaml.scores.data_portability.justification,
          evidence: p.yaml.scores.data_portability.evidence ?? null,
        },
        ai_agent_readiness: {
          value: p.scores.ai_agent_readiness,
          sub_criteria: p.yaml.scores.ai_agent_readiness.sub_criteria,
        },
      },
      bottom_line: p.yaml.bottom_line,
      gotcha: p.yaml.gotcha,
      verified_at: p.yaml.verified_at,
    })),
  };
}

export { DIMENSION_LABELS, DIMENSION_FULL_NAMES };
