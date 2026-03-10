import type { HippoPriority } from "@hippotask/core";

/**
 * Bidirectional mapping between platform-specific priority values
 * and HippoTask's normalized HippoPriority enum values.
 */
export interface PriorityMap {
  /** Platform priority value (string or number) → HippoPriority */
  toHippo: Record<string, HippoPriority>;

  /** HippoPriority → Platform priority value */
  fromHippo: Record<HippoPriority, string | number>;

  /** Fallback HippoPriority for unmapped values. Default: "none" */
  fallbackPriority: HippoPriority;
}

/**
 * Map a platform priority value to a HippoPriority.
 *
 * @returns An object with `priority` (normalized) and `priority_raw` (original value).
 */
export function mapPriority(
  platformPriority: string | number,
  map: PriorityMap,
): { priority: HippoPriority; priority_raw: string | number } {
  const key = String(platformPriority);
  const mapped = map.toHippo[key];

  if (mapped != null) {
    return { priority: mapped, priority_raw: platformPriority };
  }

  return { priority: map.fallbackPriority, priority_raw: platformPriority };
}

/**
 * Map a HippoPriority to a platform priority value.
 */
export function mapPriorityReverse(
  hippoPriority: HippoPriority,
  map: PriorityMap,
): string | number {
  return map.fromHippo[hippoPriority];
}
