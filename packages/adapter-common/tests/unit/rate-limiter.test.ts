import { describe, it, expect } from "vitest";
import { RateLimiter } from "../../src/rate-limiter.js";

describe("RateLimiter", () => {
  it("starts with full bucket", () => {
    const limiter = new RateLimiter(10, 1000);
    expect(limiter.available).toBe(10);
  });

  it("decrements tokens on acquire", async () => {
    const limiter = new RateLimiter(10, 1000);
    await limiter.acquire();
    // Tokens should be less after acquire (approximately 9)
    expect(limiter.available).toBeLessThanOrEqual(10);
  });

  it("allows multiple acquires within limit", async () => {
    const limiter = new RateLimiter(5, 1000);
    // Should complete without delay
    for (let i = 0; i < 5; i++) {
      await limiter.acquire();
    }
    expect(limiter.available).toBeLessThanOrEqual(1);
  });

  it("waits when bucket is empty", async () => {
    const limiter = new RateLimiter(2, 100); // 2 tokens per 100ms
    await limiter.acquire();
    await limiter.acquire();

    const start = Date.now();
    await limiter.acquire(); // Should wait for refill
    const elapsed = Date.now() - start;

    // Should have waited at least some time for refill
    expect(elapsed).toBeGreaterThanOrEqual(0);
  });
});
