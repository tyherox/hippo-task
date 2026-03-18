import type { Command } from "commander";
import { formatTask, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerGet(program: Command): void {
  program
    .command("get <id>")
    .description("Get a task by ID (supports partial ID prefix match)")
    .action(async (id: string, _opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        const task = await ctx.store.get(id);
        if (task === null) {
          process.stderr.write(
            formatError(`Task not found: "${id}"`, ctx.pretty) + "\n",
          );
          process.exitCode = 1;
          return;
        }
        process.stdout.write(formatTask(task, ctx.pretty) + "\n");
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
