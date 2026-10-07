import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { it } from "node:test";
import { checkEntryPoint } from "./helpers.mjs";

it("should support manual initialization", async () => {
  const mod = await import("minijinja-js/init");
  assert.equal(mod.SafeString, undefined);
  mod.initSync(
    readFileSync(new URL("../dist/wasm/minijinja_js_bg.wasm", import.meta.url)),
  );
  checkEntryPoint(mod);
});
