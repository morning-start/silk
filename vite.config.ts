import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;
const devPort = 1420;

export default defineConfig(async () => ({
  plugins: [vue(), tailwindcss()],

  clearScreen: false,
  server: {
    host: host || "127.0.0.1",
    port: devPort,
    strictPort: true,
    hmr: host ? { protocol: "ws", host, port: devPort } : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
