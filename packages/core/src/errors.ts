import type { ZodIssue } from "zod";

/**
 * Base error class for all HippoTask errors.
 */
export class HippoError extends Error {
  readonly code: string;
  readonly context?: Record<string, unknown>;

  constructor(
    code: string,
    message: string,
    context?: Record<string, unknown>,
  ) {
    super(message);
    this.name = "HippoError";
    this.code = code;
    this.context = context;
  }
}

/**
 * Schema or business rule validation failed.
 * Contains an array of issues with paths and messages.
 */
export class ValidationError extends HippoError {
  readonly issues: ZodIssue[];

  constructor(issues: ZodIssue[]) {
    const summary = issues
      .map((i) => `${i.path.join(".")}: ${i.message}`)
      .join("; ");
    super("VALIDATION_ERROR", `Validation failed: ${summary}`);
    this.name = "ValidationError";
    this.issues = issues;
  }
}
