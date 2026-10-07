import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { it } from "node:test";
import { checkEntryPoint } from "./helpers.mjs";

it("should support the default entry point", async () => {
  // The default entry point fetches the wasm module relative to itself like
  // browsers, Deno and Bun do.  Node's fetch does not support file URLs.
  const fetched = [];
  globalThis.fetch = async (url) => {
    fetched.push(String(url));
    return new Response(readFileSync(url), {
      headers: { "Content-Type": "application/wasm" },
    });
  };
  checkEntryPoint(await import("../dist/index.js"));
  assert.equal(fetched.length, 1);
  assert.ok(fetched[0].endsWith("/dist/wasm/minijinja_js_bg.wasm"));
});
