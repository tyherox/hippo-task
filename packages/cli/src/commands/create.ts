import type { Command } from "commander";
import { createTask } from "@hippotask/core";
import type { HippoStatus, HippoPriority } from "@hippotask/core";
import { appendActivity } from "../safety.js";
import { formatTask, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerCreate(program: Command): void {
  program
    .command("create <title>")
    .description("Create a new task")
    .option("-s, --status <status>", "Task status", "todo")
    .option("-p, --priority <priority>", "Task priority")
    .option("-l, --labels <labels>", "Comma-separated labels")
    .option("-d, --due <date>", "Due date (ISO 8601)")
    .option("--start <date>", "Start date (ISO 8601)")
    .option("--description <text>", "Task description")
    .option("--project <id>", "Project ID")
    .option("--parent <id>", "Parent task ID")
    .action(async (title: string, opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        let task = createTask({
          title,
          status: opts.status as HippoStatus,
          priority: opts.priority as HippoPriority | undefined,
          labels: opts.labels ? String(opts.labels).split(",").map((s: string) => s.trim()) : undefined,
          due_date: opts.due,
          start_date: opts.start,
          description: opts.description,
          project_id: opts.project,
          parent_id: opts.parent,
        });

        task = appendActivity(task, {
          agent_id: ctx.agentId,
          action: "created",
          detail: `Created task: ${title}`,
        });

        await ctx.store.create(task);
        process.stdout.write(formatTask(task, ctx.pretty) + "\n");
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
