import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  define: {
    "import.meta.env.KURA_SHELL_MACOS": JSON.stringify(
      ["darwin", "macos"].includes(
        process.env.TAURI_ENV_PLATFORM || process.platform,
      ),
    ),
  },
  resolve: {
    alias: [
      {
        find: "@",
        replacement: fileURLToPath(new URL("./src", import.meta.url)),
      },
    ],
  },
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1431 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  // Pre-bundle the packages the app imports lazily. Otherwise Vite finds
  // them on first use, re-optimizes, and open windows fail to load them
  // ("Outdated Optimize Dep") until reloaded.
  optimizeDeps: {
    include: [
      "@tauri-apps/api/webview",
      "@tauri-apps/api/window",
      "@crabnebula/tauri-plugin-drag",
      "jszip",
      "mammoth",
      "react-markdown",
    ],
  },
  build: { outDir: "dist", emptyOutDir: true, target: "es2022" },
  test: {
    environment: "jsdom",
    globals: true,
    restoreMocks: true,
    setupFiles: ["./src/tests/setup.ts"],
  },
});
