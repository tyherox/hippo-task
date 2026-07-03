import { AdapterError, AuthError, RateLimitError } from "./errors.js";

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
 * Errors that must surface immediately, without retry:
 * - AuthError (401/403)
 * - AdapterError with 4xx status codes (except 429 rate limit)
 */
function isNonRetryable(error: unknown): boolean {
  if (error instanceof AuthError) {
    return true;
  }
  return (
    error instanceof AdapterError &&
    !(error instanceof RateLimitError) &&
    error.statusCode != null &&
    error.statusCode >= 400 &&
    error.statusCode < 500
  );
}

/** Wait time before the next attempt, honoring RateLimitError.retryAfter. */
function nextWaitMs(error: unknown, fallbackMs: number): number {
  if (error instanceof RateLimitError && error.retryAfter != null) {
    return error.retryAfter * 1000;
  }
  return fallbackMs;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

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

      if (isNonRetryable(error)) {
        throw error;
      }

      // If we've exhausted retries, throw
      if (attempt >= opts.maxRetries) {
        throw lastError;
      }

      await sleep(nextWaitMs(error, delay));
      delay = Math.min(delay * opts.backoffMultiplier, opts.maxDelayMs);
    }
  }

  // Should never reach here, but TypeScript needs it
  throw lastError ?? new Error("Retry failed");
}
