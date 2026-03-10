import { describe, it, expect } from "vitest";
import { mapStatus, mapStatusReverse } from "../../src/status-map.js";
import type { StatusMap } from "../../src/status-map.js";

const TEST_MAP: StatusMap = {
  toHippo: {
    "To Do": "todo",
    "In Progress": "in_progress",
    "Done": "done",
    "Backlog": "backlog",
  },
  fromHippo: {
    backlog: "Backlog",
    todo: "To Do",
    in_progress: "In Progress",
    in_review: "In Review",
    done: "Done",
    cancelled: "Cancelled",
  },
  unmappedBehavior: "preserve",
  fallbackStatus: "todo",
};

describe("mapStatus", () => {
  it("maps a known platform status to HippoStatus", () => {
    const result = mapStatus("In Progress", TEST_MAP);
    expect(result.status).toBe("in_progress");
    expect(result.status_raw).toBe("In Progress");
  });

  it("maps all known statuses correctly", () => {
    expect(mapStatus("To Do", TEST_MAP).status).toBe("todo");
    expect(mapStatus("Done", TEST_MAP).status).toBe("done");
    expect(mapStatus("Backlog", TEST_MAP).status).toBe("backlog");
  });

  it("uses fallback for unknown status when unmappedBehavior=preserve", () => {
    const result = mapStatus("Waiting for QA", TEST_MAP);
    expect(result.status).toBe("todo"); // fallback
    expect(result.status_raw).toBe("Waiting for QA"); // preserved
  });

  it("throws for unknown status when unmappedBehavior=error", () => {
    const errorMap: StatusMap = { ...TEST_MAP, unmappedBehavior: "error" };
    expect(() => mapStatus("Unknown Status", errorMap)).toThrow(
      /Unknown platform status/,
    );
  });
});

describe("mapStatusReverse", () => {
  it("maps HippoStatus to platform status", () => {
    expect(mapStatusReverse("in_progress", TEST_MAP)).toBe("In Progress");
    expect(mapStatusReverse("done", TEST_MAP)).toBe("Done");
    expect(mapStatusReverse("cancelled", TEST_MAP)).toBe("Cancelled");
  });

  it("throws for unmapped HippoStatus", () => {
    const incompleteMap: StatusMap = {
      ...TEST_MAP,
      fromHippo: {
        backlog: "Backlog",
        todo: "To Do",
        in_progress: "In Progress",
        // Missing: in_review, done, cancelled
      } as StatusMap["fromHippo"],
    };
    expect(() => mapStatusReverse("done", incompleteMap)).toThrow(
      /No platform mapping/,
    );
  });
});
