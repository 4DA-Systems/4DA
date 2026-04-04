/**
 * 4DA MCP Server — Streamable HTTP Transport
 *
 * Implements the MCP Streamable HTTP transport (spec 2025-03-26) using
 * Node's built-in http module. No express or other framework needed.
 *
 * Security:
 * - Binds to 127.0.0.1 only (localhost)
 * - DNS rebinding protection via Origin header check
 * - Stateless mode (no session tracking)
 */

import { createServer, IncomingMessage, ServerResponse } from "node:http";
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { extractAuthClaims, type TeamClaims } from "./auth.js";

const DEFAULT_PORT = 4840;
const DEFAULT_HOST = "127.0.0.1";
const SERVER_VERSION = "4.0.1";

export interface HttpServerOptions {
  port: number;
  host: string;
}

/**
 * Parse JSON body from an incoming HTTP request.
 */
function parseBody(req: IncomingMessage): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("end", () => {
      try {
        const raw = Buffer.concat(chunks).toString("utf-8");
        resolve(raw ? JSON.parse(raw) : undefined);
      } catch (e) {
        reject(e);
      }
    });
    req.on("error", reject);
  });
}

/**
 * Start a Streamable HTTP server for the MCP protocol.
 *
 * Uses stateless mode (sessionIdGenerator: undefined) — each request
 * gets its own transport instance. This is the simplest approach and
 * is compatible with serverless environments.
 */
export async function startHttpServer(
  server: Server,
  options: HttpServerOptions = { port: DEFAULT_PORT, host: DEFAULT_HOST }
): Promise<void> {
  const { port, host } = options;

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

    // Parse JSON body for POST requests
    let body: unknown = undefined;
    if (req.method === "POST") {
      try {
        body = await parseBody(req);
      } catch {
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "Invalid JSON body" }));
        return;
      }
    }

    // Stateless mode: new transport per request
    const transport = new StreamableHTTPServerTransport({
      sessionIdGenerator: undefined,
    });

    res.on("close", () => {
      transport.close();
    });

    try {
      await server.connect(transport);
      await transport.handleRequest(req, res, body);
    } catch (err) {
      console.error("MCP transport error:", err);
      if (!res.headersSent) {
        res.writeHead(500, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "Internal server error" }));
      }
    }
  });

  httpServer.listen(port, host, () => {
    console.error(`4DA MCP Server v${SERVER_VERSION} (Streamable HTTP) listening on http://${host}:${port}/mcp`);
    console.error(`Health check: http://${host}:${port}/`);
  });

  // Graceful shutdown
  const shutdown = () => {
    console.error("Shutting down HTTP server...");
    httpServer.close();
    process.exit(0);
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}
