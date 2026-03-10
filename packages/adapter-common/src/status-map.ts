import type { HippoStatus } from "@hippotask/core";

/**
 * Bidirectional mapping between platform-specific status strings
 * and HippoTask's normalized HippoStatus enum values.
 */
export interface StatusMap {
  /** Platform status name → HippoStatus */
  toHippo: Record<string, HippoStatus>;

  /** HippoStatus → Platform status name */
  fromHippo: Record<HippoStatus, string>;

  /**
   * What to do when a platform status has no mapping.
   * "preserve" → use the platform value in status_raw, set status to fallbackStatus
   * "error" → throw an error
   */
  unmappedBehavior: "preserve" | "error";

  /** Fallback HippoStatus when unmappedBehavior is "preserve". */
  fallbackStatus: HippoStatus;
}

/**
 * Map a platform status string to a HippoStatus using the given StatusMap.
 *
 * @returns An object with `status` (normalized) and `status_raw` (original platform value).
 */
export function mapStatus(
  platformStatus: string,
  map: StatusMap,
): { status: HippoStatus; status_raw: string } {
  const mapped = map.toHippo[platformStatus];

  if (mapped != null) {
    return { status: mapped, status_raw: platformStatus };
  }

  if (map.unmappedBehavior === "error") {
    throw new Error(
      `Unknown platform status "${platformStatus}". ` +
        `Known statuses: ${Object.keys(map.toHippo).join(", ")}`,
    );
  }

  // "preserve" — use fallback status, keep raw value
  return { status: map.fallbackStatus, status_raw: platformStatus };
}

/**
 * Map a HippoStatus to a platform status string using the given StatusMap.
 */
export function mapStatusReverse(
  hippoStatus: HippoStatus,
  map: StatusMap,
): string {
  const mapped = map.fromHippo[hippoStatus];

  if (mapped != null) {
    return mapped;
  }

  throw new Error(
    `No platform mapping for HippoStatus "${hippoStatus}". ` +
      `Mapped statuses: ${Object.keys(map.fromHippo).join(", ")}`,
  );
}
