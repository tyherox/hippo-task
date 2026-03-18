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

  describe("update", () => {
    it("updates task fields", () => {
      const { stdout: createOut } = runCli('create "Original title"', storePath);
      const created = JSON.parse(createOut);
      const { stdout, exitCode } = runCli(
        `update ${created.id} --title "Updated title" --status in_progress --priority high`,
        storePath,
      );
      expect(exitCode).toBe(0);
      const task = JSON.parse(stdout);
      expect(task.title).toBe("Updated title");
      expect(task.status).toBe("in_progress");
      expect(task.priority).toBe("high");
    });

    it("logs activity on update", () => {
      const { stdout: createOut } = runCli('create "Track this" --agent bot-1', storePath);
      const created = JSON.parse(createOut);
      const { stdout } = runCli(
        `update ${created.id} --status in_progress --agent bot-1`,
        storePath,
      );
      const task = JSON.parse(stdout);
      const activity = task.metadata?.["hippotask.activity"];
      expect(activity.length).toBeGreaterThanOrEqual(2); // created + status_changed
    });

    it("returns error for non-existent task", () => {
      const { exitCode, stderr } = runCli('update fake-id --title "X"', storePath);
      expect(exitCode).toBe(1);
      expect(stderr).toContain("not found");
    });
  });

  describe("delete", () => {
    it("deletes a task", () => {
      const { stdout: createOut } = runCli('create "Doomed task"', storePath);
      const created = JSON.parse(createOut);
      const { exitCode } = runCli(`delete ${created.id}`, storePath);
      expect(exitCode).toBe(0);

      // Verify it's gone
      const { exitCode: getCode } = runCli(`get ${created.id}`, storePath);
      expect(getCode).toBe(1);
    });

    it("returns error for non-existent task", () => {
      const { exitCode, stderr } = runCli("delete fake-id", storePath);
      expect(exitCode).toBe(1);
      expect(stderr).toContain("not found");
    });
  });

  describe("done", () => {
    it("marks a task as done", () => {
      const { stdout: createOut } = runCli('create "Finish this"', storePath);
      const created = JSON.parse(createOut);
      const { stdout, exitCode } = runCli(`done ${created.id}`, storePath);
      expect(exitCode).toBe(0);
      const task = JSON.parse(stdout);
      expect(task.status).toBe("done");
      expect(task.completed_at).toBeTruthy();
    });

    it("logs completion activity", () => {
      const { stdout: createOut } = runCli('create "Complete me" --agent worker-1', storePath);
      const created = JSON.parse(createOut);
      const { stdout } = runCli(`done ${created.id} --agent worker-1`, storePath);
      const task = JSON.parse(stdout);
      const activity = task.metadata?.["hippotask.activity"];
      const completedEntry = activity.find((e: { action: string }) => e.action === "completed");
      expect(completedEntry).toBeTruthy();
    });
  });

  describe("claim + release", () => {
    it("claims a task and shows claim info", () => {
      const { stdout: createOut } = runCli('create "Claimable task"', storePath);
      const created = JSON.parse(createOut);
      const { stdout, exitCode } = runCli(
        `claim ${created.id} --agent claude-42`,
        storePath,
      );
      expect(exitCode).toBe(0);
      const task = JSON.parse(stdout);
      expect(task.metadata?.["hippotask.claimed_by"]).toBe("claude-42");
    });

    it("rejects claim by different agent", () => {
      const { stdout: createOut } = runCli('create "Contested task"', storePath);
      const created = JSON.parse(createOut);
      runCli(`claim ${created.id} --agent agent-1`, storePath);
      const { exitCode, stderr } = runCli(
        `claim ${created.id} --agent agent-2`,
        storePath,
      );
      expect(exitCode).toBe(1);
      expect(stderr).toContain("already claimed");
    });

    it("releases a claim", () => {
      const { stdout: createOut } = runCli('create "Release me"', storePath);
      const created = JSON.parse(createOut);
      runCli(`claim ${created.id} --agent agent-1`, storePath);
      const { stdout, exitCode } = runCli(
        `release ${created.id} --agent agent-1`,
        storePath,
      );
      expect(exitCode).toBe(0);
      const task = JSON.parse(stdout);
      expect(task.metadata?.["hippotask.claimed_by"]).toBeUndefined();
    });
  });

  describe("log", () => {
    it("shows activity log as JSON", () => {
      const { stdout: createOut } = runCli('create "Logged task" --agent bot-1', storePath);
      const created = JSON.parse(createOut);
      runCli(`claim ${created.id} --agent bot-1`, storePath);
      const { stdout, exitCode } = runCli(`log ${created.id}`, storePath);
      expect(exitCode).toBe(0);
      const log = JSON.parse(stdout);
      expect(log.entries.length).toBeGreaterThanOrEqual(2); // created + claimed
    });
  });

  describe("info", () => {
    it("shows store info as JSON", () => {
      runCli('create "Task A"', storePath);
      runCli('create "Task B"', storePath);
      const { stdout, exitCode } = runCli("info", storePath);
      expect(exitCode).toBe(0);
      const info = JSON.parse(stdout);
      expect(info.task_count).toBe(2);
      expect(info.store_path).toBe(storePath);
      expect(info.schema_version).toBe("1.0.0");
    });

    it("shows claim counts", () => {
      const { stdout: createOut } = runCli('create "Claimed"', storePath);
      const created = JSON.parse(createOut);
      runCli(`claim ${created.id} --agent bot-1`, storePath);
      const { stdout } = runCli("info", storePath);
      const info = JSON.parse(stdout);
      expect(info.active_claims).toBe(1);
      expect(info.claims_by_agent["bot-1"]).toBe(1);
    });
  });

  describe("init", () => {
    it("creates .hippotask directory", () => {
      const initDir = join(testDir, "project");
      mkdirSync(initDir, { recursive: true });
      const { stdout, exitCode } = runCli("init", storePath);
      expect(exitCode).toBe(0);
      expect(stdout).toContain("ok");
    });
  });
});
