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
 * - Binds to 127.0.0.1 by default. `--host` can widen that; a non-loopback
 *   bind is refused unless a shared auth secret is configured, and forces
 *   authentication on for every request.
 * - DNS rebinding protection via a `Host` header allowlist, applied to EVERY
 *   request. This is the primary guard: a rebinding attack necessarily
 *   presents the attacker's hostname in `Host`, and unlike `Origin` the header
 *   is mandatory, so a request cannot skip the check by omitting it.
 * - `Origin`, when present, must also be localhost-class. A missing `Origin`
 *   is not a pass — it is simply not additional evidence, so such requests are
 *   admitted only on the strength of the `Host` check above, and only ever
 *   without credentials on a loopback bind.
 * - Bearer tokens are HMAC-SHA256 verified (see auth.ts) before any claim is
 *   read, and the verified role is enforced on tool dispatch.
 * - Stateless (no session tracking)
 */

import { createServer, IncomingMessage, ServerResponse, type Server as HttpServer } from "node:http";
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import {
  createMcpHandler,
  localhostAllowedHostnames,
  localhostAllowedOrigins,
  validateHostHeader,
  validateOriginHeader,
  type Server,
} from "@modelcontextprotocol/server";
import { toNodeHandler } from "@modelcontextprotocol/node";
import { extractAuthClaims, getAuthSecret, type TeamClaims } from "./auth.js";
import { runWithAuthContext } from "./auth-context.js";

const DEFAULT_PORT = 4840;
const DEFAULT_HOST = "127.0.0.1";
const __ht_dirname = dirname(fileURLToPath(import.meta.url));
const SERVER_VERSION: string = JSON.parse(readFileSync(join(__ht_dirname, "..", "package.json"), "utf-8")).version;

export interface HttpServerOptions {
  port: number;
  host: string;
}

/** Handle for a running HTTP transport, so callers (and tests) can shut it down. */
export interface HttpServerHandle {
  server: HttpServer;
  close(): Promise<void>;
}

/** Security posture derived from the bind address plus environment config. */
export interface HttpSecurityPolicy {
  /** True when the server is bound to a loopback address only. */
  loopback: boolean;
  /** True when every non-health request must carry a verified Bearer token. */
  authRequired: boolean;
  /** Hostnames accepted in the `Host` header (port-agnostic). */
  allowedHostnames: string[];
  /** Hostnames accepted in the `Origin` header, when one is sent. */
  allowedOrigins: string[];
}

/** Strip IPv6 brackets and zone id so "[::1]" and "::1" compare equal. */
function normalizeHostname(host: string): string {
  return host.trim().replace(/^\[|\]$/g, "").split("%")[0].toLowerCase();
}

/** Loopback bind addresses — everything else reaches the network. */
export function isLoopbackBind(host: string): boolean {
  const h = normalizeHostname(host);
  return h === "localhost" || h === "::1" || /^127\./.test(h);
}

/**
 * Extra hostnames accepted in `Host`/`Origin`, from `MCP_ALLOWED_HOSTS`
 * (comma-separated). Needed when binding to `0.0.0.0`, where the hostname
 * legitimate clients use is not knowable from the bind address.
 */
function configuredExtraHosts(): string[] {
  return (process.env.MCP_ALLOWED_HOSTS ?? "")
    .split(",")
    .map((h) => normalizeHostname(h))
    .filter((h) => h.length > 0);
}

/**
 * Resolve the security policy for a bind address.
 *
 * A loopback bind keeps the historical behaviour: auth is opt-in via
 * `MCP_AUTH_REQUIRED`. Any other bind is network-exposed, so authentication is
 * mandatory regardless of that variable.
 */
export function resolveSecurityPolicy(host: string): HttpSecurityPolicy {
  const loopback = isLoopbackBind(host);
  const extras = configuredExtraHosts();

  // A concrete non-loopback bind address is itself a legitimate Host value;
  // wildcards (0.0.0.0 / ::) name no reachable host, so they add nothing.
  const bindHostname = normalizeHostname(host);
  const isWildcard = bindHostname === "0.0.0.0" || bindHostname === "::" || bindHostname === "";
  const bindExtras = loopback || isWildcard ? [] : [bindHostname];

  return {
    loopback,
    authRequired: loopback ? process.env.MCP_AUTH_REQUIRED === "true" : true,
    allowedHostnames: [...localhostAllowedHostnames(), ...bindExtras, ...extras],
    allowedOrigins: [...localhostAllowedOrigins(), ...bindExtras, ...extras],
  };
}

export interface GuardRejection {
  status: number;
  error: string;
}

/**
 * DNS rebinding guard. Runs on every request, before routing and before auth.
 *
 * Returns null when the request may proceed, or the rejection to send.
 */
export function evaluateRequestGuard(
  headers: { host?: string | string[]; origin?: string | string[] },
  policy: HttpSecurityPolicy,
): GuardRejection | null {
  const pick = (v: string | string[] | undefined): string | undefined =>
    Array.isArray(v) ? v[0] : v;

  // `Host` is mandatory in HTTP/1.1 and always present in HTTP/2 (as
  // :authority, which Node surfaces here). Missing means malformed, not
  // trusted — validateHostHeader rejects it.
  const hostResult = validateHostHeader(pick(headers.host), policy.allowedHostnames);
  if (!hostResult.ok) {
    return { status: 403, error: `Forbidden: DNS rebinding protection (${hostResult.errorCode})` };
  }

  // `Origin` is only sent by browsers. A present-but-foreign value is a
  // cross-origin browser request and is refused; an absent value adds no
  // evidence either way, and the Host check above has already run.
  const originResult = validateOriginHeader(pick(headers.origin), policy.allowedOrigins);
  if (!originResult.ok) {
    return { status: 403, error: `Forbidden: ${originResult.errorCode}` };
  }

  return null;
}

function sendJson(res: ServerResponse, status: number, body: unknown): void {
  res.writeHead(status, { "Content-Type": "application/json" });
  res.end(JSON.stringify(body));
}

/**
 * Refuse to start a network-exposed server that cannot authenticate anyone,
 * and make a non-loopback bind impossible to do accidentally.
 */
export function enforceBindPolicy(host: string, policy: HttpSecurityPolicy): void {
  if (policy.loopback) return;

  if (!getAuthSecret()) {
    throw new Error(
      `Refusing to bind the MCP HTTP transport to a non-loopback address (${host}) with no auth secret.\n` +
        `  Every 4DA tool (dependency inventory, decisions, agent memory, Developer DNA) would be\n` +
        `  readable and writable by anyone who can reach this port.\n` +
        `  Set MCP_AUTH_SECRET (the same value as the relay's JWT_SECRET) to enable token verification,\n` +
        `  or drop --host and serve on 127.0.0.1.`,
    );
  }

  console.error("");
  console.error("  ############################################################");
  console.error(`  # WARNING: MCP HTTP transport is bound to ${host}`);
  console.error("  # This exposes 4DA tools beyond this machine.");
  console.error("  # Authentication is REQUIRED for every request on this bind.");
  console.error("  # Put it behind TLS and a trusted network boundary.");
  console.error("  ############################################################");
  console.error("");

  const extras = configuredExtraHosts();
  const bindHostname = normalizeHostname(host);
  if ((bindHostname === "0.0.0.0" || bindHostname === "::") && extras.length === 0) {
    console.error(
      "  NOTE: MCP_ALLOWED_HOSTS is unset, so only localhost-class Host headers are accepted.",
    );
    console.error(
      "        Set MCP_ALLOWED_HOSTS=<hostname-or-ip[,...]> to the address clients actually use.",
    );
    console.error("");
  }
}

/**
 * Start a Streamable HTTP server for the MCP protocol.
 *
 * Takes a server factory: `createMcpHandler` builds a fresh instance per
 * request (stateless — compatible with serverless environments and simple
 * round-robin load balancing).
 *
 * Resolves once the server is accepting connections.
 */
export async function startHttpServer(
  factory: () => Server,
  options: HttpServerOptions = { port: DEFAULT_PORT, host: DEFAULT_HOST },
): Promise<HttpServerHandle> {
  const { port, host } = options;
  const policy = resolveSecurityPolicy(host);
  enforceBindPolicy(host, policy);

  const mcpHandler = createMcpHandler(factory);
  const handleMcpRequest = toNodeHandler(mcpHandler, {
    onerror: (err) => {
      console.error("MCP transport error:", err);
    },
  });

  const httpServer = createServer(async (req: IncomingMessage, res: ServerResponse) => {
    // DNS rebinding protection — every request, before routing or auth.
    const rejection = evaluateRequestGuard(req.headers, policy);
    if (rejection) {
      sendJson(res, rejection.status, { error: rejection.error });
      return;
    }

    const pathname = new URL(req.url || "/", `http://${host}:${port}`).pathname;

    // Auth gate. Unauthenticated health checks on "/" stay open so liveness
    // probes work; the endpoint reveals only name, version and status.
    let claims: TeamClaims | null = null;
    if (policy.authRequired && pathname !== "/") {
      claims = extractAuthClaims(req);
      if (!claims) {
        res.writeHead(401, {
          "Content-Type": "application/json",
          "WWW-Authenticate": 'Bearer realm="4da-mcp"',
        });
        res.end(JSON.stringify({ error: "Authentication required" }));
        return;
      }
    }

    // Health check endpoint
    if (pathname === "/" && req.method === "GET") {
      sendJson(res, 200, {
        name: "4da-mcp",
        version: SERVER_VERSION,
        transport: "streamable-http",
        status: "ok",
      });
      return;
    }

    // Only handle /mcp endpoint
    if (pathname !== "/mcp") {
      sendJson(res, 404, { error: "Not Found" });
      return;
    }

    // Hand off to the MCP handler — body parsing, era classification, and
    // per-request instance construction happen inside the entry. The auth
    // context rides along so tool dispatch can enforce the verified role.
    await runWithAuthContext({ claims, enforced: policy.authRequired }, () =>
      handleMcpRequest(req, res),
    );
  });

  await new Promise<void>((resolve, reject) => {
    httpServer.once("error", reject);
    httpServer.listen(port, host, () => {
      httpServer.removeListener("error", reject);
      resolve();
    });
  });

  console.error(`4DA MCP Server v${SERVER_VERSION} (Streamable HTTP) listening on http://${host}:${port}/mcp`);
  console.error(`Health check: http://${host}:${port}/`);
  console.error(
    `Authentication: ${policy.authRequired ? "required (HMAC-SHA256 verified Bearer tokens)" : "disabled (loopback only)"}`,
  );

  const close = async (): Promise<void> => {
    await mcpHandler.close();
    await new Promise<void>((resolve) => httpServer.close(() => resolve()));
  };

  // Graceful shutdown
  const shutdown = () => {
    console.error("Shutting down HTTP server...");
    void close().finally(() => process.exit(0));
  };
  process.once("SIGINT", shutdown);
  process.once("SIGTERM", shutdown);

  return { server: httpServer, close };
}
