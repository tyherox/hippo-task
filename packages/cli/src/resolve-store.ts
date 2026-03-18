import { existsSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";

/**
 * Resolve the task store file path.
 *
 * Resolution order:
 * 1. Explicit `flagPath` (--store CLI flag)
 * 2. HIPPOTASK_STORE environment variable
 * 3. `.hippotask/tasks.json` in `cwd` (per-project store)
 * 4. `~/.hippotask/tasks.json` (global fallback)
 */
export function resolveStorePath(
  flagPath?: string,
  cwd?: string,
): string {
  // 1. Explicit flag
  if (flagPath != null && flagPath.length > 0) {
    return flagPath;
  }

  // 2. Environment variable
  const envPath = process.env["HIPPOTASK_STORE"];
  if (envPath != null && envPath.length > 0) {
    return envPath;
  }

  // 3. Local project store
  const resolvedCwd = cwd ?? process.cwd();
  const localDir = join(resolvedCwd, ".hippotask");
  if (existsSync(localDir)) {
    return join(localDir, "tasks.json");
  }

  // 4. Global store
  return join(homedir(), ".hippotask", "tasks.json");
}
