import { Command } from "commander";
import { registerCreate } from "./commands/create.js";
import { registerList } from "./commands/list.js";
import { registerGet } from "./commands/get.js";
import { registerUpdate } from "./commands/update.js";
import { registerDelete } from "./commands/delete.js";
import { registerDone } from "./commands/done.js";
import { registerClaim } from "./commands/claim.js";
import { registerRelease } from "./commands/release.js";
import { registerLog } from "./commands/log.js";
import { registerInit } from "./commands/init.js";
import { registerInfo } from "./commands/info.js";

const program = new Command();

program
  .name("hippotask")
  .description("Task management for humans and AI agents")
  .version("0.1.0")
  .option("--store <path>", "Path to task store file")
  .option("--agent <id>", "Agent identifier for claims and activity log")
  .option("--pretty", "Human-readable output instead of JSON");

// Register all commands
registerCreate(program);
registerList(program);
registerGet(program);
registerUpdate(program);
registerDelete(program);
registerDone(program);
registerClaim(program);
registerRelease(program);
registerLog(program);
registerInit(program);
registerInfo(program);

export function run(argv: string[]): void {
  program.parse(argv);
}

export { program };
