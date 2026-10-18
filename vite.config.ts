import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import process from "node:process";
import { fileURLToPath, URL } from "node:url";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [vue(), tailwindcss()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  // tauri prints rust errors to the same terminal; vite would wipe them
  clearScreen: false,
  server: {
    // tauri.conf.json devUrl is pinned to this port
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: { environment: "node" },
  // shipped inside the app and read from disk, so one big chunk costs nothing at runtime
  build: { chunkSizeWarningLimit: 2500 },
}));
