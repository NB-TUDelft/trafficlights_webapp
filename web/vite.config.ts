// SPDX-License-Identifier: AGPL-3.0-or-later
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The Rust server (`cargo run -p trafficlights-server`) listens here by default.
const backend = "http://127.0.0.1:3000";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    port: 5173,
    proxy: {
      "/api": backend,
      // WebSockets: `/ws/lobby` and `/ws/sessions/{id}`.
      "/ws": { target: backend, ws: true },
    },
  },
});
