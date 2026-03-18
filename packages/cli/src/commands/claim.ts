import type { Command } from "commander";
import { claimTask } from "../safety.js";
import { formatTask, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerClaim(program: Command): void {
  program
    .command("claim <id>")
    .description("Claim a task for the current agent")
    .option("--ttl <seconds>", "Claim TTL in seconds", "3600")
    .action(async (id: string, opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        const current = await ctx.store.get(id);
        if (current === null) {
          process.stderr.write(formatError(`Task not found: "${id}"`, ctx.pretty) + "\n");
          process.exitCode = 1;
          return;
        }

        const claimed = claimTask(current, ctx.agentId, parseInt(String(opts.ttl), 10));

        // Write claim metadata back to store
        await ctx.store.update(current.id, { metadata: claimed.metadata });

        // Read back to get updated_at
        const updated = await ctx.store.get(current.id);
        process.stdout.write(formatTask(updated ?? claimed, ctx.pretty) + "\n");
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
