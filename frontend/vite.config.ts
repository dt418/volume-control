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
    rollupOptions: {
      input: {
        root: resolve(import.meta.dirname, "index.html"),
        mixer: resolve(import.meta.dirname, "src/mixer/index.html"),
        settings: resolve(import.meta.dirname, "src/settings/index.html"),
        help: resolve(import.meta.dirname, "src/help/index.html"),
      },
    },
  },
  test: { environment: "jsdom", setupFiles: ["src/test-setup.ts"] },
});
