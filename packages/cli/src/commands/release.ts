import type { Command } from "commander";
import { releaseTask } from "../safety.js";
import { formatTask, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerRelease(program: Command): void {
  program
    .command("release <id>")
    .description("Release a task claim")
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

        const released = releaseTask(current, ctx.agentId);

        // Write metadata back to store
        await ctx.store.update(current.id, { metadata: released.metadata });

        const updated = await ctx.store.get(current.id);
        process.stdout.write(formatTask(updated ?? released, ctx.pretty) + "\n");
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
