/**
 * Generate a UUIDv7 identifier.
 *
 * UUIDv7 (RFC 9562) embeds a Unix timestamp in the high bits,
 * making IDs time-sortable and globally unique.
 *
 * Falls back to crypto.randomUUID() on environments where
 * manual UUIDv7 construction isn't needed for sorting.
 */
export function generateId(): string {
  // UUIDv7: timestamp (48 bits) + version (4 bits) + random (12 bits) + variant (2 bits) + random (62 bits)
  const timestamp = Date.now();

  // Encode timestamp into 6 bytes (48 bits)
  const timeHex = timestamp.toString(16).padStart(12, "0");

  // Generate random bytes for the rest
  const randomBytes = new Uint8Array(10);
  crypto.getRandomValues(randomBytes);

  // Build UUID string
  // Positions: tttttttt-tttt-7xxx-yxxx-xxxxxxxxxxxx
  // where t = timestamp, 7 = version, y = variant (8/9/a/b), x = random
  const randomHex = Array.from(randomBytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");

  const uuid = [
    timeHex.slice(0, 8),                                    // 8 chars
    timeHex.slice(8, 12),                                    // 4 chars
    `7${randomHex.slice(0, 3)}`,                             // 4 chars (version 7)
    `${((parseInt(randomHex.slice(3, 5), 16) & 0x3f) | 0x80).toString(16).padStart(2, "0")}${randomHex.slice(5, 7)}`, // 4 chars (variant 10xx)
    randomHex.slice(7, 19),                                  // 12 chars
  ].join("-");

  return uuid;
}
