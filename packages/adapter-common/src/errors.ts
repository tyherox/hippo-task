import { HippoError } from "@hippotask/core";

/**
 * Platform API call failed.
 */
export class AdapterError extends HippoError {
  readonly platform: string;
  readonly statusCode?: number;

  constructor(
    platform: string,
    message: string,
    statusCode?: number,
    context?: Record<string, unknown>,
  ) {
    super("ADAPTER_ERROR", `[${platform}] ${message}`, context);
    this.name = "AdapterError";
    this.platform = platform;
    this.statusCode = statusCode;
  }
}

/**
 * Rate limit exceeded (HTTP 429).
 */
export class RateLimitError extends AdapterError {
  override readonly code = "RATE_LIMIT_ERROR";
  readonly retryAfter?: number;

  constructor(platform: string, retryAfter?: number) {
    super(
      platform,
      retryAfter != null
        ? `Rate limit exceeded. Retry after ${retryAfter}s.`
        : "Rate limit exceeded.",
      429,
    );
    this.name = "RateLimitError";
    this.retryAfter = retryAfter;
  }
}

/**
 * Task not found on platform (HTTP 404).
 */
export class NotFoundError extends AdapterError {
  override readonly code = "NOT_FOUND";
  readonly externalId: string;

  constructor(platform: string, externalId: string) {
    super(platform, `Task "${externalId}" not found.`, 404);
    this.name = "NotFoundError";
    this.externalId = externalId;
  }
}

/**
 * Authentication failed (HTTP 401/403).
 */
export class AuthError extends AdapterError {
  override readonly code = "AUTH_ERROR";

  constructor(platform: string, message?: string) {
    super(platform, message ?? "Authentication failed.", 401);
    this.name = "AuthError";
  }
}
