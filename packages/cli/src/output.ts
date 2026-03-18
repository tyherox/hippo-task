import type { HippoTask, PaginatedResult } from "@hippotask/core";

/**
 * Format output for the CLI.
 * Default: JSON (machine-readable for AI agents).
 * With --pretty: human-readable table/summary.
 */
export function formatTask(task: HippoTask, pretty: boolean): string {
  if (!pretty) {
    return JSON.stringify(task);
  }
  const priority = task.priority ? ` [${task.priority}]` : "";
  const labels = task.labels?.length ? ` (${task.labels.join(", ")})` : "";
  const due = task.due_date ? ` due:${task.due_date}` : "";
  return `${task.id.slice(0, 8)}  ${task.status.padEnd(12)} ${task.title}${priority}${labels}${due}`;
}

export function formatTaskList(
  result: PaginatedResult<HippoTask>,
  pretty: boolean,
): string {
  if (!pretty) {
    return JSON.stringify(result);
  }
  if (result.items.length === 0) {
    return "No tasks found.";
  }
  const lines = result.items.map((t) => formatTask(t, true));
  const header = `${result.items.length} task(s)${result.has_more ? " (more available)" : ""}:\n`;
  return header + lines.join("\n");
}

export function formatSuccess(message: string, data: unknown, pretty: boolean): string {
  if (!pretty) {
    return JSON.stringify({ ok: true, message, ...( typeof data === "object" && data !== null ? data : { data }) });
  }
  return `✅ ${message}`;
}

export function formatError(error: unknown, pretty: boolean): string {
  const message = error instanceof Error ? error.message : String(error);
  if (!pretty) {
    return JSON.stringify({ ok: false, error: message });
  }
  return `❌ ${message}`;
}
