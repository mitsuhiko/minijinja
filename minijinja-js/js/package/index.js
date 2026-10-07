// Default entry point for browsers, bundlers, Deno and Bun.  The wasm
// module is loaded relative to this module (via `import.meta.url`) and
// initialized with top-level await.
import init from "./wasm/minijinja_js.js";
import { setup } from "./shared.js";

await init();
setup();

export {
  Environment,
  SafeString,
  TemplateError,
  markSafe,
} from "./shared.js";
