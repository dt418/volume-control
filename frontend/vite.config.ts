import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    // Vite 8 runs Rolldown. The plugin-timing report is informational only:
    // the measured hook here is Vite's own build-html transform, so there is
    // no plugin we can remove to improve it. Disable that one check to keep
    // release logs signal-only; real warnings stay enabled.
    rolldownOptions: {
      checks: { pluginTimings: false },
      input: {
        root: resolve(import.meta.dirname, "index.html"),
        mixer: resolve(import.meta.dirname, "src/mixer/index.html"),
        settings: resolve(import.meta.dirname, "src/settings/index.html"),
        help: resolve(import.meta.dirname, "src/help/index.html"),
        overlay: resolve(import.meta.dirname, "src/overlay/index.html"),
      },
    },
  },
  test: { environment: "jsdom", setupFiles: ["src/test-setup.ts"] },
});
