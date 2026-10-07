// Each entry point is tested in a separate file so that every test runs
// in its own process with a fresh wasm module.
import { createRequire } from "node:module";
import { it } from "node:test";
import { checkEntryPoint } from "./helpers.mjs";

it("should support importing the node entry point", async () => {
  checkEntryPoint(await import("minijinja-js"));
});

it("should support requiring the node entry point", () => {
  const require = createRequire(import.meta.url);
  checkEntryPoint(require("minijinja-js"));
});
