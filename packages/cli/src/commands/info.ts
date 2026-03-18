import type { Command } from "commander";
import { formatError } from "../output.js";
import { buildContext } from "../context.js";
import { isTaskClaimed, getClaimInfo } from "../safety.js";

export function registerInfo(program: Command): void {
  program
    .command("info")
    .description("Show store location, task count, and schema version")
    .action(async (_opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        const count = await ctx.store.count();
        const result = await ctx.store.list({ limit: 200 });

        const activeClaims = result.items.filter(isTaskClaimed);
        const claimsByAgent: Record<string, number> = {};
        for (const task of activeClaims) {
          const info = getClaimInfo(task);
          if (info) {
            claimsByAgent[info.claimed_by] = (claimsByAgent[info.claimed_by] ?? 0) + 1;
          }
        }

        const statusCounts: Record<string, number> = {};
        for (const task of result.items) {
          statusCounts[task.status] = (statusCounts[task.status] ?? 0) + 1;
        }

        const info = {
          store_path: ctx.storePath,
          agent_id: ctx.agentId,
          task_count: count,
          active_claims: activeClaims.length,
          claims_by_agent: claimsByAgent,
          status_counts: statusCounts,
          schema_version: "1.0.0",
        };

        if (!ctx.pretty) {
          process.stdout.write(JSON.stringify(info) + "\n");
        } else {
          process.stdout.write(`HippoTask Store Info:\n`);
          process.stdout.write(`  Store: ${info.store_path}\n`);
          process.stdout.write(`  Agent: ${info.agent_id}\n`);
          process.stdout.write(`  Tasks: ${info.task_count}\n`);
          process.stdout.write(`  Active claims: ${info.active_claims}\n`);
          if (Object.keys(info.status_counts).length > 0) {
            process.stdout.write(`  Status breakdown: ${JSON.stringify(info.status_counts)}\n`);
          }
          if (Object.keys(info.claims_by_agent).length > 0) {
            process.stdout.write(`  Claims by agent: ${JSON.stringify(info.claims_by_agent)}\n`);
          }
        }
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
