import { RateLimitError, AdapterError, AuthError } from "./errors.js";

/**
 * Options for the retry utility.
 */
export interface RetryOptions {
  /** Maximum number of retry attempts. Default: 3. */
  maxRetries?: number;

  /** Initial delay in ms before the first retry. Default: 1000. */
  initialDelayMs?: number;

  /** Multiplier applied to delay after each retry. Default: 2. */
  backoffMultiplier?: number;

  /** Maximum delay in ms between retries. Default: 30000. */
  maxDelayMs?: number;
}

const DEFAULT_OPTIONS: Required<RetryOptions> = {
  maxRetries: 3,
  initialDelayMs: 1000,
  backoffMultiplier: 2,
  maxDelayMs: 30000,
};

/**
 * Execute a function with exponential backoff retry.
 *
 * Retries on:
 * - RateLimitError (uses retryAfter if available)
 * - AdapterError with 5xx status codes
 * - Generic errors (assumed transient)
 *
 * Does NOT retry:
 * - AuthError (401/403) — surface immediately
 * - AdapterError with 4xx status codes (except 429)
 */
export async function withRetry<T>(
  fn: () => Promise<T>,
  options?: RetryOptions,
): Promise<T> {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  let lastError: Error | undefined;
  let delay = opts.initialDelayMs;

  for (let attempt = 0; attempt <= opts.maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));

      // Never retry auth errors
      if (error instanceof AuthError) {
        throw error;
      }

      // Never retry 4xx errors (except 429 rate limit)
      if (
        error instanceof AdapterError &&
        error.statusCode != null &&
        error.statusCode >= 400 &&
        error.statusCode < 500 &&
        !(error instanceof RateLimitError)
      ) {
        throw error;
      }

      // If we've exhausted retries, throw
      if (attempt >= opts.maxRetries) {
        throw lastError;
      }

      // Determine wait time
      let waitMs = delay;
      if (error instanceof RateLimitError && error.retryAfter != null) {
        waitMs = error.retryAfter * 1000;
      }

      await new Promise((resolve) => setTimeout(resolve, waitMs));
      delay = Math.min(delay * opts.backoffMultiplier, opts.maxDelayMs);
    }
  }

  // Should never reach here, but TypeScript needs it
  throw lastError ?? new Error("Retry failed");
}
