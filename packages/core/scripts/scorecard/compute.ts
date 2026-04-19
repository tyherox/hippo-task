/**
 * Score computation from validated provider YAML.
 *
 * Sum-dimensions (D1, D2, D6) count met sub-criteria, floor 1, cap 5.
 * D8 uses per-sub-criterion points (MCP = 2, others = 1), floor 1, cap 5.
 * D4 is derived from effective_ops_per_min via bandFromOpsMin.
 * D3, D5, D7 use the explicit band stated in the YAML.
 * Overall = unweighted mean of 8 dimensions, rounded to 1 decimal.
 */

import {
  D8_POINTS,
  type ProviderYAML,
  type SubCriterionT,
  bandFromOpsMin,
} from "./schema.js";

export interface ComputedScores {
  readonly api_completeness: number;
  readonly api_ergonomics: number;
  readonly schema_flexibility: number;
  readonly rate_limits: number;
  readonly real_time: number;
  readonly auth_simplicity: number;
  readonly data_portability: number;
  readonly ai_agent_readiness: number;
  readonly overall: number;
}

function sumDimension(subCriteria: readonly SubCriterionT[]): number {
  const raw = subCriteria.reduce((acc, c) => acc + (c.met ? 1 : 0), 0);
  return Math.max(1, Math.min(5, raw));
}

function d8Dimension(subCriteria: readonly SubCriterionT[]): number {
  const raw = subCriteria.reduce((acc, c) => {
    if (!c.met) return acc;
    const points = D8_POINTS[c.id as keyof typeof D8_POINTS] ?? 1;
    return acc + points;
  }, 0);
  return Math.max(1, Math.min(5, raw));
}

export function computeScores(provider: ProviderYAML): ComputedScores {
  const s = provider.scores;
  const scores = {
    api_completeness: sumDimension(s.api_completeness.sub_criteria),
    api_ergonomics: sumDimension(s.api_ergonomics.sub_criteria),
    schema_flexibility: s.schema_flexibility.band,
    rate_limits: bandFromOpsMin(s.rate_limits.effective_ops_per_min),
    real_time: s.real_time.band,
    auth_simplicity: sumDimension(s.auth_simplicity.sub_criteria),
    data_portability: s.data_portability.band,
    ai_agent_readiness: d8Dimension(s.ai_agent_readiness.sub_criteria),
  };

  const sum = Object.values(scores).reduce((a, b) => a + b, 0);
  const overall = Math.round((sum / 8) * 10) / 10;

  return { ...scores, overall };
}
