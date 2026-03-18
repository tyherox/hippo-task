import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { execSync } from "node:child_process";

/** Helper: run CLI command and return { stdout, stderr, exitCode } */
function runCli(
  args: string,
  storePath: string,
): { stdout: string; stderr: string; exitCode: number } {
  const binPath = join(__dirname, "..", "..", "dist", "bin.js");
  const cmd = `node ${binPath} --store ${storePath} ${args}`;
  try {
    const stdout = execSync(cmd, { encoding: "utf-8", timeout: 10000 });
    return { stdout, stderr: "", exitCode: 0 };
  } catch (error: unknown) {
    const execError = error as { stdout?: string; stderr?: string; status?: number };
    return {
      stdout: execError.stdout ?? "",
      stderr: execError.stderr ?? "",
      exitCode: execError.status ?? 1,
    };
  }
}

describe("CLI Commands", () => {
  let testDir: string;
  let storePath: string;

  beforeEach(() => {
    testDir = join(tmpdir(), `hippotask-cli-test-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`);
    mkdirSync(testDir, { recursive: true });
    storePath = join(testDir, "tasks.json");
  });

  afterEach(() => {
    rmSync(testDir, { recursive: true, force: true });
  });

  describe("create", () => {
    it("creates a task and outputs JSON", () => {
      const { stdout, exitCode } = runCli('create "Fix login bug"', storePath);
      expect(exitCode).toBe(0);
      const task = JSON.parse(stdout);
      expect(task.title).toBe("Fix login bug");
      expect(task.status).toBe("todo");
      expect(task.id).toBeTruthy();
      expect(task.schema_version).toBe("1.0.0");
    });

    it("creates a task with options", () => {
      const { stdout, exitCode } = runCli(
        'create "Ship feature" --status in_progress --priority high --labels "frontend,auth"',
        storePath,
      );
      expect(exitCode).toBe(0);
      const task = JSON.parse(stdout);
      expect(task.status).toBe("in_progress");
      expect(task.priority).toBe("high");
      expect(task.labels).toEqual(["frontend", "auth"]);
    });

    it("includes activity log entry", () => {
      const { stdout } = runCli(
        'create "With activity" --agent test-agent',
        storePath,
      );
      const task = JSON.parse(stdout);
      const activity = task.metadata?.["hippotask.activity"];
      expect(Array.isArray(activity)).toBe(true);
      expect(activity[0]?.action).toBe("created");
    });
  });

  describe("list", () => {
    it("lists tasks as JSON", () => {
      runCli('create "Task A"', storePath);
      runCli('create "Task B"', storePath);
      const { stdout, exitCode } = runCli("list", storePath);
      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.items).toHaveLength(2);
      expect(result.has_more).toBe(false);
    });

    it("filters by status", () => {
      runCli('create "Todo task" --status todo', storePath);
      runCli('create "Done task" --status done', storePath);
      const { stdout } = runCli("list --status done", storePath);
      const result = JSON.parse(stdout);
      expect(result.items).toHaveLength(1);
      expect(result.items[0].title).toBe("Done task");
    });

    it("returns empty list for empty store", () => {
      const { stdout, exitCode } = runCli("list", storePath);
      expect(exitCode).toBe(0);
      const result = JSON.parse(stdout);
      expect(result.items).toHaveLength(0);
    });

    it("searches by title", () => {
      runCli('create "Fix login bug"', storePath);
      runCli('create "Add dashboard"', storePath);
      const { stdout } = runCli('list --search "login"', storePath);
      const result = JSON.parse(stdout);
      expect(result.items).toHaveLength(1);
      expect(result.items[0].title).toBe("Fix login bug");
    });
  });

  describe("get", () => {
    it("gets a task by full ID", () => {
      const { stdout: createOut } = runCli('create "My task"', storePath);
      const created = JSON.parse(createOut);
      const { stdout, exitCode } = runCli(`get ${created.id}`, storePath);
      expect(exitCode).toBe(0);
      const task = JSON.parse(stdout);
      expect(task.title).toBe("My task");
      expect(task.id).toBe(created.id);
    });

    it("gets a task by ID prefix", () => {
      const { stdout: createOut } = runCli('create "Prefix test"', storePath);
      const created = JSON.parse(createOut);
      const prefix = created.id.slice(0, 8);
      const { stdout, exitCode } = runCli(`get ${prefix}`, storePath);
      expect(exitCode).toBe(0);
      const task = JSON.parse(stdout);
      expect(task.title).toBe("Prefix test");
    });

    it("returns error for non-existent ID", () => {
      const { exitCode, stderr } = runCli("get nonexistent-id", storePath);
      expect(exitCode).toBe(1);
      expect(stderr).toContain("not found");
    });
  });
});
