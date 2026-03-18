import type { Command } from "commander";
import type { HippoStatus, HippoPriority } from "@hippotask/core";
import { isTaskClaimed, getClaimInfo } from "../safety.js";
import { formatTaskList, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerList(program: Command): void {
  program
    .command("list")
    .description("List tasks with optional filters")
    .option("-s, --status <statuses>", "Filter by status (comma-separated)")
    .option("-p, --priority <priority>", "Filter by priority")
    .option("-l, --labels <labels>", "Filter by labels (comma-separated)")
    .option("--project <id>", "Filter by project ID")
    .option("--search <text>", "Search in title and description")
    .option("--limit <n>", "Max results", "50")
    .option("--unclaimed", "Show only unclaimed tasks")
    .option("--claimed-by <agent>", "Show tasks claimed by a specific agent")
    .action(async (opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        const statusFilter = opts.status
          ? String(opts.status).split(",").map((s: string) => s.trim()) as HippoStatus[]
          : undefined;

        const result = await ctx.store.list({
          status: statusFilter,
          priority: opts.priority as HippoPriority | undefined,
          labels: opts.labels ? String(opts.labels).split(",").map((s: string) => s.trim()) : undefined,
          project_id: opts.project,
          search: opts.search,
          limit: parseInt(String(opts.limit), 10),
        });

        // Apply claim filters (post-query since claims are in metadata)
        if (opts.unclaimed) {
          result.items = result.items.filter((t) => !isTaskClaimed(t));
        }
        if (opts.claimedBy) {
          result.items = result.items.filter((t) => {
            const info = getClaimInfo(t);
            return info !== null && info.claimed_by === opts.claimedBy;
          });
        }

        process.stdout.write(formatTaskList(result, ctx.pretty) + "\n");
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
