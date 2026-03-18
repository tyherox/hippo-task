import type { Command } from "commander";
import { mkdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import { formatSuccess, formatError } from "../output.js";
import { buildContext } from "../context.js";

export function registerInit(program: Command): void {
  program
    .command("init")
    .description("Initialize .hippotask/ in the current directory")
    .action((_opts, cmd) => {
      const globalOpts = cmd.parent?.opts() ?? {};
      const ctx = buildContext(globalOpts);
      try {
        const dir = join(process.cwd(), ".hippotask");
        if (existsSync(dir)) {
          process.stdout.write(
            formatSuccess("Already initialized", { path: dir }, ctx.pretty) + "\n",
          );
          return;
        }
        mkdirSync(dir, { recursive: true });
        process.stdout.write(
          formatSuccess(`Initialized HippoTask store at ${dir}`, { path: dir }, ctx.pretty) + "\n",
        );
      } catch (error) {
        process.stderr.write(formatError(error, ctx.pretty) + "\n");
        process.exitCode = 1;
      }
    });
}
