import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import http from "node:http";

/**
 * Dev-only proxy so the browser build can reach a real Palworld server.
 * Browsers can't call the REST API directly (CORS + HTTP Basic preflight), so the
 * frontend posts to `/__palapi__/*` with the target host/port/password in headers,
 * and this middleware forwards to `http://<host>:<port>/v1/api/*` with the Basic
 * auth header attached. The Tauri desktop app uses its Rust backend and never hits this.
 */
function palApiProxy(): Plugin {
  return {
    name: "pal-api-proxy",
    configureServer(server) {
      server.middlewares.use("/__palapi__", (req, res) => {
        const host = String(req.headers["x-pal-host"] || "localhost");
        const port = Number(req.headers["x-pal-port"]) || 8212;
        const pass = String(req.headers["x-pal-pass"] || "");
        const headers: Record<string, string> = {
          authorization: "Basic " + Buffer.from("admin:" + pass).toString("base64"),
          accept: "application/json",
        };
        const contentType = req.headers["content-type"];
        if (contentType) headers["content-type"] = String(contentType);

        const upstream = http.request(
          { host, port, path: "/v1/api" + (req.url || ""), method: req.method, headers, timeout: 8000 },
          (r) => {
            res.statusCode = r.statusCode || 502;
            res.setHeader("content-type", String(r.headers["content-type"] || "application/json"));
            r.pipe(res);
          },
        );
        upstream.on("timeout", () => upstream.destroy(new Error("timeout")));
        upstream.on("error", () => {
          if (!res.headersSent) res.statusCode = 502;
          res.end('{"error":"Could not reach the server."}');
        });
        req.pipe(upstream);
      });
    },
  };
}

// Tauri expects a fixed dev port and no clearing of the terminal.
export default defineConfig({
  plugins: [react(), palApiProxy()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "esnext",
    chunkSizeWarningLimit: 1200,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
