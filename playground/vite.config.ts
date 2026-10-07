import { execSync } from "node:child_process";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

function commit(): string {
  if (process.env.GITHUB_SHA) {
    return process.env.GITHUB_SHA;
  }
  try {
    return execSync("git rev-parse HEAD", { encoding: "utf8" }).trim();
  } catch {
    return "";
  }
}

export default defineConfig({
  // relative paths so that the build works under any path (eg: GitHub Pages)
  base: "./",
  plugins: [react()],
  define: {
    "import.meta.env.VITE_COMMIT": JSON.stringify(commit()),
  },
  build: {
    target: "es2022",
    chunkSizeWarningLimit: 1500,
  },
  worker: {
    format: "es",
  },
  server: {
    fs: {
      // allow serving the minijinja-js package from the repository
      allow: [".."],
    },
  },
});
