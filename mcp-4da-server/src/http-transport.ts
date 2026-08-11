// SPDX-License-Identifier: Apache-2.0
/**
 * 4DA MCP Server — Streamable HTTP Transport
 *
 * Serves MCP over HTTP using the v2 `createMcpHandler` entry (one factory,
 * one endpoint, both protocol eras): 2026-07-28 requests are served natively;
 * 2025-era requests get the established stateless serving (fresh instance per
 * request — the same behavior as the previous per-request
 * `StreamableHTTPServerTransport` wiring). Node's built-in http module only;
 * no express or other framework needed.
 *
 * Security:
 * - Binds to 127.0.0.1 only (localhost)
 * - DNS rebinding protection via Origin header check
 * - Stateless (no session tracking)
 */

import { createServer, IncomingMessage, ServerResponse } from "node:http";
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createMcpHandler, type Server } from "@modelcontextprotocol/server";
import { toNodeHandler } from "@modelcontextprotocol/node";
import { extractAuthClaims, type TeamClaims } from "./auth.js";

const DEFAULT_PORT = 4840;
const DEFAULT_HOST = "127.0.0.1";
const __ht_dirname = dirname(fileURLToPath(import.meta.url));
const SERVER_VERSION: string = JSON.parse(readFileSync(join(__ht_dirname, "..", "package.json"), "utf-8")).version;

export interface HttpServerOptions {
  port: number;
  host: string;
}

/**
 * Start a Streamable HTTP server for the MCP protocol.
 *
 * Takes a server factory: `createMcpHandler` builds a fresh instance per
 * request (stateless — compatible with serverless environments and simple
 * round-robin load balancing).
 */
export async function startHttpServer(
  factory: () => Server,
  options: HttpServerOptions = { port: DEFAULT_PORT, host: DEFAULT_HOST }
): Promise<void> {
  const { port, host } = options;

  const mcpHandler = createMcpHandler(factory);
  const handleMcpRequest = toNodeHandler(mcpHandler, {
    onerror: (err) => {
      console.error("MCP transport error:", err);
    },
  });

  const httpServer = createServer(async (req: IncomingMessage, res: ServerResponse) => {
    // DNS rebinding protection: reject cross-origin requests from non-local origins
    const origin = req.headers.origin;
    if (origin) {
      try {
        const url = new URL(origin);
        if (url.hostname !== "localhost" && url.hostname !== "127.0.0.1" && url.hostname !== "::1" && url.hostname !== "[::1]") {
          res.writeHead(403, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "Forbidden: DNS rebinding protection" }));
          return;
        }
      } catch {
        res.writeHead(403, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "Forbidden: invalid Origin header" }));
        return;
      }
    }

    // Auth check: if MCP_AUTH_REQUIRED env var is set, validate JWT
    const authRequired = process.env.MCP_AUTH_REQUIRED === "true";
    let claims: TeamClaims | null = null;

    if (authRequired) {
      // Allow unauthenticated health checks on root
      const checkPath = new URL(req.url || "/", `http://${host}:${port}`).pathname;
      if (checkPath !== "/") {
        claims = extractAuthClaims(req);
        if (!claims) {
          res.writeHead(401, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "Authentication required" }));
          return;
        }
      }
    }

    // Route requests
    const pathname = new URL(req.url || "/", `http://${host}:${port}`).pathname;

    // Health check endpoint
    if (pathname === "/" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({
        name: "4da-mcp",
        version: SERVER_VERSION,
        transport: "streamable-http",
        status: "ok",
      }));
      return;
    }

    // Only handle /mcp endpoint
    if (pathname !== "/mcp") {
      res.writeHead(404, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: "Not Found" }));
      return;
    }

    // Hand off to the MCP handler — body parsing, era classification, and
    // per-request instance construction happen inside the entry.
    await handleMcpRequest(req, res);
  });

  httpServer.listen(port, host, () => {
    console.error(`4DA MCP Server v${SERVER_VERSION} (Streamable HTTP) listening on http://${host}:${port}/mcp`);
    console.error(`Health check: http://${host}:${port}/`);
  });

  // Graceful shutdown
  const shutdown = () => {
    console.error("Shutting down HTTP server...");
    void mcpHandler.close();
    httpServer.close();
    process.exit(0);
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
