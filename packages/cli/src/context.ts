import { hostname } from "node:os";
import { FileTaskStore } from "./store.js";
import { resolveStorePath } from "./resolve-store.js";

/**
 * CLI context — resolved from global flags.
 * Passed to every command handler.
 */
export interface CliContext {
  store: FileTaskStore;
  agentId: string;
  pretty: boolean;
  storePath: string;
}

/**
 * Build CLI context from global options.
 */
export function buildContext(opts: {
  store?: string;
  agent?: string;
  pretty?: boolean;
}): CliContext {
  const storePath = resolveStorePath(opts.store);
  return {
    store: new FileTaskStore(storePath),
    agentId: opts.agent ?? process.env["HIPPOTASK_AGENT"] ?? hostname(),
    pretty: opts.pretty ?? false,
    storePath,
  };
}
