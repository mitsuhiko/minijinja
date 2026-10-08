/**
 * A string that is marked as safe and is not auto escaped.
 */
export class SafeString extends String {}

/**
 * An error raised by the template engine.
 *
 * Callbacks can throw template errors to fail with a specific kind and
 * without the "JavaScript function threw" prefix.
 */
export class TemplateError extends Error {
  constructor(message, info = {}) {
    super(
      message,
      info.cause === undefined ? undefined : { cause: info.cause },
    );
    this.name = "TemplateError";
    this.kind = info.kind ?? "InvalidOperation";
    this.detail = info.detail;
    this.templateName = info.templateName;
    this.line = info.line;
    this.range = info.range;
    this.templateSource = info.templateSource;
  }
}

/**
 * Returns the support classes so that they can be re-exported.
 */
export function getSupportClasses() {
  return { SafeString, TemplateError };
}
