// Builds the package into `dist/`.
//
// The wasm module is built once for the `web` target into `dist/wasm/` and
// the hand written entry points from `js/package/` are copied next to it.
//
// Usage: node scripts/build.mjs [--dev]
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const dev = process.argv.includes("--dev");

rmSync(dist, { recursive: true, force: true });

const result = spawnSync(
  "wasm-pack",
  [
    "build",
    "--target",
    "web",
    "--out-dir",
    "dist/wasm",
    "--out-name",
    "minijinja_js",
    "--no-pack",
    dev ? "--dev" : "--release",
  ],
  {
    cwd: root,
    stdio: "inherit",
    shell: process.platform === "win32",
    env: {
      ...process.env,
      // The release profile is shared with the rest of the workspace, so
      // the size optimizations for the wasm build are configured here.
      // These make the module about a third smaller at no measurable cost
      // in render performance.
      CARGO_PROFILE_RELEASE_OPT_LEVEL: "z",
      CARGO_PROFILE_RELEASE_LTO: "true",
      CARGO_PROFILE_RELEASE_CODEGEN_UNITS: "1",
    },
  }
);
if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

for (const file of [".gitignore", "README.md", "package.json"]) {
  const path = `${dist}wasm/${file}`;
  if (existsSync(path)) {
    rmSync(path);
  }
}

cpSync(fileURLToPath(new URL("../js/package/", import.meta.url)), dist, {
  recursive: true,
});
