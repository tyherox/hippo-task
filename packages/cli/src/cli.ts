import { Command } from "commander";

const program = new Command();

program
  .name("hippotask")
  .description("Task management for humans and AI agents")
  .version("0.1.0")
  .option("--store <path>", "Path to task store file")
  .option("--agent <id>", "Agent identifier for claims and activity log")
  .option("--pretty", "Human-readable output instead of JSON");

export function run(argv: string[]): void {
  program.parse(argv);
}

export { program };
