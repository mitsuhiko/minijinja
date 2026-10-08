// Entry point for manual initialization.  This is useful for environments
// where the wasm module needs to be provided explicitly (for instance
// Cloudflare Workers or custom asset pipelines).
import wasmInit, { initSync as wasmInitSync } from "./wasm/minijinja_js.js";
import { setup } from "./shared.js";

export async function init(moduleOrPath) {
  await wasmInit(
    moduleOrPath === undefined ? undefined : { module_or_path: moduleOrPath },
  );
  setup();
}

export function initSync(module) {
  wasmInitSync({ module });
  setup();
}

export default init;

export {
  Environment,
  SafeString,
  TemplateError,
  markSafe,
  passState,
} from "./shared.js";
