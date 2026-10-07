export { Environment } from "./wasm/minijinja_js.js";
export type {
  AutoEscape,
  Context,
  SyntaxConfig,
  UndefinedBehavior,
} from "./wasm/minijinja_js.js";

/**
 * A string that is marked as safe and is not auto escaped.
 *
 * Safe strings are passed to callbacks as `SafeString` objects and callbacks
 * can return them to emit markup.  As `SafeString` extends `String`, all
 * string methods are available but return plain (unsafe) strings.
 */
export declare class SafeString extends String {
  constructor(value: string);
}

/**
 * Marks a string as safe.
 */
export declare function markSafe(value: string): SafeString;

/**
 * An error raised by the template engine.
 */
export declare class TemplateError extends Error {
  name: "TemplateError";
  /** The kind of error (for instance `"SyntaxError"` or `"UndefinedError"`). */
  kind: string;
  /** The detail message of the error, if available. */
  detail?: string;
  /** The name of the template the error happened in, if available. */
  templateName?: string;
  /** The line number (1-based) of the error, if available. */
  line?: number;
  /** The byte range of the error in the template source, if available. */
  range?: { start: number; end: number };
  /** The source of the template the error happened in, if available. */
  templateSource?: string;
  /** The exception thrown by a JavaScript callback, if any. */
  cause?: unknown;
}
