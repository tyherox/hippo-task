import type { Command } from "commander";
import { formatSuccess, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerDelete(program: Command): void {
  program
    .command("delete <id>")
    .description("Delete a task")
    .action(async (id: string, _opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        const task = await ctx.store.get(id);
        if (task === null) {
          process.stderr.write(formatError(`Task not found: "${id}"`, ctx.pretty) + "\n");
          process.exitCode = 1;
          return;
        }

        await ctx.store.delete(task.id);
        process.stdout.write(
          formatSuccess(`Deleted task "${task.title}" (${task.id.slice(0, 8)})`, { id: task.id }, ctx.pretty) + "\n",
        );
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
