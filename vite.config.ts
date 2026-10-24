import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// ---------------------------------------------------------
// Keep the Vite development server aligned with Tauri.
// ---------------------------------------------------------
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || "localhost",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
