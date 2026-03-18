import type { Command } from "commander";
import type { HippoStatus, HippoPriority } from "@hippotask/core";
import { appendActivity } from "../safety.js";
import { formatTask, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerUpdate(program: Command): void {
  program
    .command("update <id>")
    .description("Update a task (optimistic lock enforced)")
    .option("-t, --title <title>", "New title")
    .option("-s, --status <status>", "New status")
    .option("-p, --priority <priority>", "New priority")
    .option("-l, --labels <labels>", "New labels (comma-separated)")
    .option("-d, --due <date>", "New due date")
    .option("--start <date>", "New start date")
    .option("--description <text>", "New description")
    .option("--project <id>", "New project ID")
    .option("--parent <id>", "New parent task ID")
    .action(async (id: string, opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        // Read current task for optimistic locking
        const current = await ctx.store.get(id);
        if (current === null) {
          process.stderr.write(formatError(`Task not found: "${id}"`, ctx.pretty) + "\n");
          process.exitCode = 1;
          return;
        }

        // Build the partial update
        const changes: Record<string, unknown> = {};
        const changedFields: string[] = [];

        if (opts.title) { changes["title"] = opts.title; changedFields.push("title"); }
        if (opts.status) { changes["status"] = opts.status as HippoStatus; changedFields.push("status"); }
        if (opts.priority) { changes["priority"] = opts.priority as HippoPriority; changedFields.push("priority"); }
        if (opts.labels) { changes["labels"] = String(opts.labels).split(",").map((s: string) => s.trim()); changedFields.push("labels"); }
        if (opts.due) { changes["due_date"] = opts.due; changedFields.push("due_date"); }
        if (opts.start) { changes["start_date"] = opts.start; changedFields.push("start_date"); }
        if (opts.description) { changes["description"] = opts.description; changedFields.push("description"); }
        if (opts.project) { changes["project_id"] = opts.project; changedFields.push("project_id"); }
        if (opts.parent) { changes["parent_id"] = opts.parent; changedFields.push("parent_id"); }

        if (changedFields.length === 0) {
          process.stderr.write(formatError("No fields to update. Use --title, --status, etc.", ctx.pretty) + "\n");
          process.exitCode = 1;
          return;
        }

        // Build activity detail
        const detail = changedFields.join(", ") + " updated";
        if (opts.status && opts.status !== current.status) {
          changes["_activity_detail"] = `${current.status} → ${opts.status}`;
        }

        // Perform optimistic-locked update
        let updated = await ctx.store.update(
          current.id,
          changes,
          current.updated_at, // Optimistic lock: reject if task changed since we read it
        );

        // Log activity
        updated = appendActivity(updated, {
          agent_id: ctx.agentId,
          action: opts.status ? "status_changed" : "updated",
          detail: opts.status && opts.status !== current.status
            ? `${current.status} → ${opts.status}`
            : detail,
        });

        // Write activity back (another store update without optimistic lock)
        updated = await ctx.store.update(current.id, {
          metadata: updated.metadata,
        });

        process.stdout.write(formatTask(updated, ctx.pretty) + "\n");
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
