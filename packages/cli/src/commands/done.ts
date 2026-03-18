import type { Command } from "commander";
import { nowISO } from "@hippotask/core";
import { appendActivity, releaseTask, isTaskClaimed, getClaimInfo } from "../safety.js";
import { formatTask, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerDone(program: Command): void {
  program
    .command("done <id>")
    .description("Mark a task as done (shortcut for update --status done)")
    .action(async (id: string, _opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        const current = await ctx.store.get(id);
        if (current === null) {
          process.stderr.write(formatError(`Task not found: "${id}"`, ctx.pretty) + "\n");
          process.exitCode = 1;
          return;
        }

        const previousStatus = current.status;

        // Update status + completed_at
        let updated = await ctx.store.update(
          current.id,
          { status: "done", completed_at: nowISO() },
          current.updated_at,
        );

        // Log activity
        updated = appendActivity(updated, {
          agent_id: ctx.agentId,
          action: "completed",
          detail: `${previousStatus} → done`,
        });

        // Auto-release claim if this agent owns it
        if (isTaskClaimed(updated)) {
          const claim = getClaimInfo(updated);
          if (claim?.claimed_by === ctx.agentId) {
            updated = releaseTask(updated, ctx.agentId);
          }
        }

        // Write metadata changes back
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
