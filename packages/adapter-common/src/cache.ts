/**
 * Simple TTL (time-to-live) cache.
 *
 * Useful for caching platform API responses to avoid
 * hitting rate limits on frequently-accessed data.
 */
export class TtlCache<T> {
  private readonly store = new Map<string, { value: T; expiresAt: number }>();
  private readonly defaultTtlMs: number;

  /**
   * @param defaultTtlMs Default time-to-live in milliseconds. Default: 60000 (1 minute).
   */
  constructor(defaultTtlMs: number = 60_000) {
    this.defaultTtlMs = defaultTtlMs;
  }

  /**
   * Get a value from the cache.
   * Returns undefined if not found or expired.
   */
  get(key: string): T | undefined {
    const entry = this.store.get(key);
    if (entry == null) return undefined;

    if (Date.now() > entry.expiresAt) {
      this.store.delete(key);
      return undefined;
    }

    return entry.value;
  }

  /**
   * Set a value in the cache.
   * @param ttlMs Optional TTL override for this entry.
   */
  set(key: string, value: T, ttlMs?: number): void {
    const ttl = ttlMs ?? this.defaultTtlMs;
    this.store.set(key, {
      value,
      expiresAt: Date.now() + ttl,
    });
  }

  /**
   * Check if a key exists and is not expired.
   */
  has(key: string): boolean {
    return this.get(key) !== undefined;
  }

  /**
   * Delete a specific key.
   */
  delete(key: string): boolean {
    return this.store.delete(key);
  }

  /**
   * Clear all entries.
   */
  clear(): void {
    this.store.clear();
  }

  /**
   * Number of entries (including potentially expired ones).
   */
  get size(): number {
    return this.store.size;
  }
}
