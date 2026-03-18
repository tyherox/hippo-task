import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { resolveStorePath } from "../../src/resolve-store.js";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";

vi.mock("node:fs", async () => {
  const actual = await vi.importActual<typeof import("node:fs")>("node:fs");
  return { ...actual, existsSync: vi.fn() };
});

const mockedExistsSync = vi.mocked(existsSync);

describe("resolveStorePath", () => {
  const originalEnv = process.env["HIPPOTASK_STORE"];

  beforeEach(() => {
    delete process.env["HIPPOTASK_STORE"];
    mockedExistsSync.mockReset();
  });

  afterEach(() => {
    if (originalEnv !== undefined) {
      process.env["HIPPOTASK_STORE"] = originalEnv;
    } else {
      delete process.env["HIPPOTASK_STORE"];
    }
  });

  it("uses explicit flag value first", () => {
    const result = resolveStorePath("/explicit/path/tasks.json");
    expect(result).toBe("/explicit/path/tasks.json");
  });

  it("uses HIPPOTASK_STORE env var second", () => {
    process.env["HIPPOTASK_STORE"] = "/env/path/tasks.json";
    const result = resolveStorePath(undefined);
    expect(result).toBe("/env/path/tasks.json");
  });

  it("uses local .hippotask/tasks.json if it exists", () => {
    mockedExistsSync.mockImplementation((p) => {
      return String(p).includes(".hippotask");
    });
    const result = resolveStorePath(undefined, "/project/root");
    expect(result).toBe(join("/project/root", ".hippotask", "tasks.json"));
  });

  it("falls back to global ~/.hippotask/tasks.json", () => {
    mockedExistsSync.mockReturnValue(false);
    const result = resolveStorePath(undefined, "/project/root");
    expect(result).toBe(join(homedir(), ".hippotask", "tasks.json"));
  });
});
