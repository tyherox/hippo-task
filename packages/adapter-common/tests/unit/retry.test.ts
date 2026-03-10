import { describe, it, expect, vi } from "vitest";
import { withRetry } from "../../src/retry.js";
import { AuthError, AdapterError, RateLimitError, NotFoundError } from "../../src/errors.js";

describe("withRetry", () => {
  it("returns result on first success", async () => {
    const fn = vi.fn().mockResolvedValue("success");
    const result = await withRetry(fn, { maxRetries: 3 });
    expect(result).toBe("success");
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("retries on transient errors and succeeds", async () => {
    const fn = vi
      .fn()
      .mockRejectedValueOnce(new Error("transient"))
      .mockResolvedValue("recovered");

    const result = await withRetry(fn, {
      maxRetries: 3,
      initialDelayMs: 10,
    });

    expect(result).toBe("recovered");
    expect(fn).toHaveBeenCalledTimes(2);
  });

  it("throws after exhausting retries", async () => {
    const fn = vi.fn().mockRejectedValue(new Error("always fails"));

    await expect(
      withRetry(fn, { maxRetries: 2, initialDelayMs: 10 }),
    ).rejects.toThrow("always fails");

    expect(fn).toHaveBeenCalledTimes(3); // initial + 2 retries
  });

  it("does NOT retry AuthError", async () => {
    const fn = vi
      .fn()
      .mockRejectedValue(new AuthError("jira", "Bad token"));

    await expect(
      withRetry(fn, { maxRetries: 3, initialDelayMs: 10 }),
    ).rejects.toThrow(AuthError);

    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("does NOT retry NotFoundError (4xx)", async () => {
    const fn = vi
      .fn()
      .mockRejectedValue(new NotFoundError("github", "repo#999"));

    await expect(
      withRetry(fn, { maxRetries: 3, initialDelayMs: 10 }),
    ).rejects.toThrow(NotFoundError);

    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("DOES retry RateLimitError (429)", async () => {
    const fn = vi
      .fn()
      .mockRejectedValueOnce(new RateLimitError("notion", 0.01))
      .mockResolvedValue("ok");

    const result = await withRetry(fn, {
      maxRetries: 3,
      initialDelayMs: 10,
    });

    expect(result).toBe("ok");
    expect(fn).toHaveBeenCalledTimes(2);
  });

  it("retries 5xx AdapterError", async () => {
    const fn = vi
      .fn()
      .mockRejectedValueOnce(new AdapterError("jira", "Server error", 500))
      .mockResolvedValue("recovered");

    const result = await withRetry(fn, {
      maxRetries: 2,
      initialDelayMs: 10,
    });

    expect(result).toBe("recovered");
    expect(fn).toHaveBeenCalledTimes(2);
  });
});
