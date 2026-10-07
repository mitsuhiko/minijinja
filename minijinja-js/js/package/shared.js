// Internal module shared by all entry points.  The support classes are
// provided by the wasm module and can only be retrieved once it has been
// initialized, which is why they are exported as live bindings.
import { Environment, __getSupportClasses } from "./wasm/minijinja_js.js";

export let SafeString;
export let TemplateError;

export function setup() {
  ({ SafeString, TemplateError } = __getSupportClasses());
}

export function markSafe(value) {
  return new SafeString(value);
}

export { Environment };
