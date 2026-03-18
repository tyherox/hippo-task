import type { Command } from "commander";
import { getActivityLog } from "../safety.js";
import { formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerLog(program: Command): void {
  program
    .command("log <id>")
    .description("Show activity log for a task")
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

        const log = getActivityLog(task);

        if (!ctx.pretty) {
          process.stdout.write(JSON.stringify({ task_id: task.id, entries: log }) + "\n");
        } else {
          if (log.length === 0) {
            process.stdout.write("No activity recorded.\n");
          } else {
            process.stdout.write(`Activity log for ${task.id.slice(0, 8)} "${task.title}":\n`);
            for (const entry of log) {
              const detail = entry.detail ? ` — ${entry.detail}` : "";
              process.stdout.write(
                `  ${entry.timestamp}  ${entry.agent_id}  ${entry.action}${detail}\n`,
              );
            }
          }
        }
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
