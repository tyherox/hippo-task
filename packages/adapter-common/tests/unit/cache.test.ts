import { describe, it, expect, vi } from "vitest";
import { TtlCache } from "../../src/cache.js";

describe("TtlCache", () => {
  it("stores and retrieves a value", () => {
    const cache = new TtlCache<string>();
    cache.set("key", "value");
    expect(cache.get("key")).toBe("value");
  });

  it("returns undefined for missing key", () => {
    const cache = new TtlCache<string>();
    expect(cache.get("nonexistent")).toBeUndefined();
  });

  it("returns undefined for expired key", () => {
    const cache = new TtlCache<string>(50); // 50ms TTL
    cache.set("key", "value");

    // Fast-forward time
    vi.useFakeTimers();
    vi.advanceTimersByTime(100);
    expect(cache.get("key")).toBeUndefined();
    vi.useRealTimers();
  });

  it("respects per-entry TTL override", () => {
    const cache = new TtlCache<string>(60_000); // 1 minute default
    cache.set("short", "value", 50); // 50ms override

    vi.useFakeTimers();
    vi.advanceTimersByTime(100);
    expect(cache.get("short")).toBeUndefined();
    vi.useRealTimers();
  });

  it("has() returns true for existing non-expired key", () => {
    const cache = new TtlCache<string>();
    cache.set("key", "value");
    expect(cache.has("key")).toBe(true);
  });

  it("has() returns false for missing key", () => {
    const cache = new TtlCache<string>();
    expect(cache.has("key")).toBe(false);
  });

  it("delete() removes a key", () => {
    const cache = new TtlCache<string>();
    cache.set("key", "value");
    cache.delete("key");
    expect(cache.get("key")).toBeUndefined();
  });

  it("clear() removes all keys", () => {
    const cache = new TtlCache<string>();
    cache.set("a", "1");
    cache.set("b", "2");
    cache.clear();
    expect(cache.size).toBe(0);
  });

  it("size reflects number of entries", () => {
    const cache = new TtlCache<string>();
    expect(cache.size).toBe(0);
    cache.set("a", "1");
    cache.set("b", "2");
    expect(cache.size).toBe(2);
  });
});
