// Entry point for Node.js.  The wasm module is loaded synchronously from
// disk so that this module does not need top-level await and can also be
// loaded with `require()`.
import { readFileSync } from "node:fs";
import { initSync } from "./wasm/minijinja_js.js";
import { setup } from "./shared.js";

initSync({
  module: readFileSync(new URL("./wasm/minijinja_js_bg.wasm", import.meta.url)),
});
setup();

export {
  Environment,
  SafeString,
  TemplateError,
  markSafe,
  passState,
} from "./shared.js";
