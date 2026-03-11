/**
 * Token bucket rate limiter.
 *
 * Allows `maxTokens` requests per `intervalMs`. Tokens refill gradually.
 * Call `acquire()` before making a request — it resolves when a token
 * is available (may wait if bucket is empty).
 */
export class RateLimiter {
  private tokens: number;
  private readonly maxTokens: number;
  private readonly refillRateMs: number;
  private lastRefill: number;

  /**
   * @param maxTokens Maximum tokens (requests) in the bucket.
   * @param intervalMs Time window in ms for the token limit.
   */
  constructor(maxTokens: number, intervalMs: number) {
    this.maxTokens = maxTokens;
    this.tokens = maxTokens;
    this.refillRateMs = intervalMs / maxTokens;
    this.lastRefill = Date.now();
  }

  /**
   * Acquire a token. Resolves immediately if available,
   * or waits until a token is available.
   */
  async acquire(): Promise<void> {
    this.refill();

    if (this.tokens >= 1) {
      this.tokens -= 1;
      return;
    }

    // Wait for the next token
    const waitMs = this.refillRateMs - (Date.now() - this.lastRefill);
    const safeWait = Math.max(waitMs, 1);
    await new Promise((resolve) => setTimeout(resolve, safeWait));

    // Refill and try again
    this.refill();
    this.tokens = Math.max(this.tokens - 1, 0);
  }

  /** Current number of available tokens. */
  get available(): number {
    this.refill();
    return Math.floor(this.tokens);
  }

  private refill(): void {
    const now = Date.now();
    const elapsed = now - this.lastRefill;
    const newTokens = elapsed / this.refillRateMs;
    this.tokens = Math.min(this.tokens + newTokens, this.maxTokens);
    this.lastRefill = now;
  }
}
