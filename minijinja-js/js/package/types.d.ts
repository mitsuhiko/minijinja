import type { State } from "./wasm/minijinja_js.js";

export { Environment } from "./wasm/minijinja_js.js";
export type {
  AutoEscape,
  Context,
  State,
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
 * Marks a callback to receive the engine `State` as first argument.
 *
 * ```js
 * env.addFilter("greet", passState((state, name) =>
 *   `${state.lookup("greeting") ?? "Hello"} ${name}`));
 * ```
 *
 * The state is only valid while the callback runs.
 */
export declare function passState<
  F extends (state: State, ...args: any[]) => any,
>(func: F): F;

/**
 * An error raised by the template engine.
 *
 * Callbacks can throw template errors to fail with a specific kind:
 *
 * ```js
 * throw new TemplateError("value must be positive", { kind: "InvalidOperation" });
 * ```
 */
export declare class TemplateError extends Error {
  constructor(
    message: string,
    info?: { kind?: string; detail?: string; cause?: unknown },
  );
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
