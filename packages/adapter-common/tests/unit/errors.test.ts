import { describe, it, expect } from "vitest";
import {
  AdapterError,
  RateLimitError,
  NotFoundError,
  AuthError,
} from "../../src/errors.js";
import { HippoError } from "@hippotask/core";

describe("AdapterError", () => {
  it("extends HippoError", () => {
    const error = new AdapterError("jira", "Something failed", 500);
    expect(error).toBeInstanceOf(HippoError);
    expect(error).toBeInstanceOf(Error);
  });

  it("includes platform and status code", () => {
    const error = new AdapterError("jira", "Failed", 500);
    expect(error.platform).toBe("jira");
    expect(error.statusCode).toBe(500);
    expect(error.code).toBe("ADAPTER_ERROR");
    expect(error.message).toContain("jira");
    expect(error.message).toContain("Failed");
  });
});

describe("RateLimitError", () => {
  it("extends AdapterError with rate limit info", () => {
    const error = new RateLimitError("notion", 30);
    expect(error).toBeInstanceOf(AdapterError);
    expect(error.code).toBe("RATE_LIMIT_ERROR");
    expect(error.statusCode).toBe(429);
    expect(error.retryAfter).toBe(30);
    expect(error.platform).toBe("notion");
  });

  it("works without retryAfter", () => {
    const error = new RateLimitError("clickup");
    expect(error.retryAfter).toBeUndefined();
    expect(error.message).toContain("Rate limit exceeded");
  });
});

describe("NotFoundError", () => {
  it("includes external ID", () => {
    const error = new NotFoundError("github", "owner/repo#42");
    expect(error.code).toBe("NOT_FOUND");
    expect(error.statusCode).toBe(404);
    expect(error.externalId).toBe("owner/repo#42");
    expect(error.message).toContain("owner/repo#42");
  });
});

describe("AuthError", () => {
  it("defaults to 401", () => {
    const error = new AuthError("linear");
    expect(error.code).toBe("AUTH_ERROR");
    expect(error.statusCode).toBe(401);
    expect(error.message).toContain("Authentication failed");
  });

  it("accepts custom message", () => {
    const error = new AuthError("jira", "Token expired");
    expect(error.message).toContain("Token expired");
  });
});
