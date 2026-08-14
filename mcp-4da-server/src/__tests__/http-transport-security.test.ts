// SPDX-License-Identifier: Apache-2.0
/**
 * Security regression tests for the Streamable HTTP transport.
 *
 * Three defects shipped together in v5.0.0:
 *
 *   1. The file claimed "Binds to 127.0.0.1 only" while `--host` accepted any
 *      address, so `--http --host 0.0.0.0` served every tool to the network.
 *   2. The DNS rebinding guard was `if (origin) { ...check... }` — a request
 *      with no Origin header (curl, any non-browser client, and any attacker)
 *      skipped it entirely.
 *   3. Auth was optional even on a network bind.
 *
 * These tests lock the fixes: the Host allowlist runs on EVERY request
 * (Origin present or not), a non-loopback bind is refused without a configured
 * secret, and a network bind forces authentication on regardless of
 * MCP_AUTH_REQUIRED.
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { request as httpRequest, type IncomingHttpHeaders } from "node:http";
import { createHmac } from "node:crypto";
import { Server } from "@modelcontextprotocol/server";

import {
  enforceBindPolicy,
  evaluateRequestGuard,
  isLoopbackBind,
  resolveSecurityPolicy,
  startHttpServer,
  type HttpSecurityPolicy,
  type HttpServerHandle,
} from "../http-transport.js";

const SECRET = "relay-shared-secret-for-tests";

const AUTH_ENV = ["MCP_AUTH_SECRET", "MCP_AUTH_REQUIRED", "MCP_ALLOWED_HOSTS", "JWT_SECRET"] as const;
const savedEnv: Record<string, string | undefined> = {};

/** Recomputed per test so it never inherits the operator's environment. */
let LOOPBACK_POLICY: HttpSecurityPolicy;

// The transport logs its banner to stderr; keep the test output readable.
const realError = console.error;

beforeEach(() => {
  for (const key of AUTH_ENV) {
    savedEnv[key] = process.env[key];
    delete process.env[key];
  }
  LOOPBACK_POLICY = resolveSecurityPolicy("127.0.0.1");
  console.error = () => {};
});

afterEach(() => {
  console.error = realError;
  for (const key of AUTH_ENV) {
    if (savedEnv[key] === undefined) delete process.env[key];
    else process.env[key] = savedEnv[key];
  }
});

describe("bind classification", () => {
  it("recognises loopback addresses", () => {
    for (const host of ["127.0.0.1", "127.0.0.2", "localhost", "::1", "[::1]", "LOCALHOST"]) {
      expect(isLoopbackBind(host), host).toBe(true);
    }
  });

  it("recognises network-exposing addresses", () => {
    for (const host of ["0.0.0.0", "::", "192.168.1.10", "10.0.0.5", "example.com"]) {
      expect(isLoopbackBind(host), host).toBe(false);
    }
  });
});

describe("security policy", () => {
  it("keeps auth opt-in on loopback", () => {
    expect(resolveSecurityPolicy("127.0.0.1").authRequired).toBe(false);
    process.env.MCP_AUTH_REQUIRED = "true";
    expect(resolveSecurityPolicy("127.0.0.1").authRequired).toBe(true);
  });

  it("FORCES auth on for any non-loopback bind, even with MCP_AUTH_REQUIRED unset", () => {
    expect(resolveSecurityPolicy("0.0.0.0").authRequired).toBe(true);
    process.env.MCP_AUTH_REQUIRED = "false";
    expect(resolveSecurityPolicy("192.168.1.10").authRequired).toBe(true);
  });

  it("allowlists only localhost-class hostnames by default", () => {
    expect(resolveSecurityPolicy("127.0.0.1").allowedHostnames).toEqual([
      "localhost",
      "127.0.0.1",
      "[::1]",
    ]);
  });

  it("adds a concrete non-loopback bind address to the allowlist", () => {
    expect(resolveSecurityPolicy("192.168.1.10").allowedHostnames).toContain("192.168.1.10");
  });

  it("does NOT allowlist the wildcard address itself", () => {
    expect(resolveSecurityPolicy("0.0.0.0").allowedHostnames).not.toContain("0.0.0.0");
  });

  it("honours MCP_ALLOWED_HOSTS for wildcard binds", () => {
    process.env.MCP_ALLOWED_HOSTS = "mcp.internal, 10.0.0.5 ";
    const policy = resolveSecurityPolicy("0.0.0.0");
    expect(policy.allowedHostnames).toContain("mcp.internal");
    expect(policy.allowedHostnames).toContain("10.0.0.5");
    expect(policy.allowedOrigins).toContain("mcp.internal");
  });
});

describe("DNS rebinding guard", () => {
  it("REJECTS a rebinding Host even when the request sends no Origin (the v5.0.0 bypass)", () => {
    const rejection = evaluateRequestGuard({ host: "attacker.example:4840" }, LOOPBACK_POLICY);
    expect(rejection?.status).toBe(403);
  });

  it("REJECTS a request with no Host header at all", () => {
    expect(evaluateRequestGuard({}, LOOPBACK_POLICY)?.status).toBe(403);
  });

  it("ACCEPTS localhost-class Host headers with or without a port", () => {
    for (const host of ["localhost:4840", "127.0.0.1:4840", "127.0.0.1", "[::1]:4840"]) {
      expect(evaluateRequestGuard({ host }, LOOPBACK_POLICY), host).toBeNull();
    }
  });

  it("REJECTS a foreign Origin even when the Host header is fine", () => {
    const rejection = evaluateRequestGuard(
      { host: "127.0.0.1:4840", origin: "https://evil.example" },
      LOOPBACK_POLICY,
    );
    expect(rejection?.status).toBe(403);
  });

  it("REJECTS an opaque (null) Origin", () => {
    const rejection = evaluateRequestGuard(
      { host: "127.0.0.1:4840", origin: "null" },
      LOOPBACK_POLICY,
    );
    expect(rejection?.status).toBe(403);
  });

  it("ACCEPTS a localhost Origin", () => {
    expect(
      evaluateRequestGuard(
        { host: "127.0.0.1:4840", origin: "http://localhost:5173" },
        LOOPBACK_POLICY,
      ),
    ).toBeNull();
  });

  it("REJECTS a malformed Host header", () => {
    expect(evaluateRequestGuard({ host: "not a host" }, LOOPBACK_POLICY)?.status).toBe(403);
  });
});

describe("bind policy enforcement", () => {
  it("REFUSES to start on a non-loopback address with no auth secret", async () => {
    // Rejects before any socket is opened.
    await expect(startHttpServer(dummyFactory, { port: 0, host: "0.0.0.0" })).rejects.toThrow(
      /non-loopback/i,
    );
  });

  it("explains how to fix the refusal", () => {
    expect(() => enforceBindPolicy("0.0.0.0", resolveSecurityPolicy("0.0.0.0"))).toThrow(
      /MCP_AUTH_SECRET/,
    );
  });

  it("permits a non-loopback bind once a secret is configured", () => {
    process.env.MCP_AUTH_SECRET = SECRET;
    expect(() => enforceBindPolicy("0.0.0.0", resolveSecurityPolicy("0.0.0.0"))).not.toThrow();
  });

  it("never blocks a loopback bind", () => {
    expect(() => enforceBindPolicy("127.0.0.1", resolveSecurityPolicy("127.0.0.1"))).not.toThrow();
  });
});

// --- Live server integration -------------------------------------------------

function dummyFactory(): Server {
  return new Server({ name: "test", version: "0.0.0" }, { capabilities: { tools: {} } });
}

interface RawResponse {
  status: number;
  body: string;
  headers: IncomingHttpHeaders;
}

function rawRequest(
  port: number,
  options: { method?: string; path?: string; headers?: Record<string, string>; body?: string },
): Promise<RawResponse> {
  return new Promise((resolve, reject) => {
    const req = httpRequest(
      {
        host: "127.0.0.1",
        port,
        method: options.method ?? "GET",
        path: options.path ?? "/",
        // setHost:false lets us send a Host header the client would never
        // normally forge — exactly what a DNS rebinding victim's browser does.
        setHost: false,
        headers: { host: `127.0.0.1:${port}`, ...options.headers },
      },
      (res) => {
        let body = "";
        res.setEncoding("utf-8");
        res.on("data", (chunk: string) => (body += chunk));
        res.on("end", () =>
          resolve({ status: res.statusCode ?? 0, body, headers: res.headers }),
        );
      },
    );
    req.on("error", reject);
    if (options.body) req.write(options.body);
    req.end();
  });
}

const b64url = (value: unknown): string =>
  Buffer.from(JSON.stringify(value)).toString("base64url");

/** A genuinely signed token for the configured test secret. */
function signedToken(role: string): string {
  const header = b64url({ alg: "HS256", typ: "JWT" });
  const payload = b64url({
    team_id: "team-alpha",
    client_id: "client-1",
    role,
    exp: 9999999999,
  });
  const signature = createHmac("sha256", SECRET)
    .update(`${header}.${payload}`)
    .digest("base64url");
  return `${header}.${payload}.${signature}`;
}

describe("live HTTP server with auth required", () => {
  let handle: HttpServerHandle;
  let port: number;

  beforeEach(async () => {
    process.env.MCP_AUTH_SECRET = SECRET;
    process.env.MCP_AUTH_REQUIRED = "true";
    handle = await startHttpServer(dummyFactory, { port: 0, host: "127.0.0.1" });
    const address = handle.server.address();
    port = typeof address === "object" && address !== null ? address.port : 0;
  });

  afterEach(async () => {
    await handle.close();
  });

  it("serves the health check unauthenticated", async () => {
    const res = await rawRequest(port, { path: "/" });
    expect(res.status).toBe(200);
    expect(JSON.parse(res.body).status).toBe("ok");
  });

  it("REJECTS a rebinding Host on the health check too", async () => {
    const res = await rawRequest(port, { path: "/", headers: { host: "attacker.example" } });
    expect(res.status).toBe(403);
  });

  it("REJECTS an unauthenticated /mcp request that sends no Origin", async () => {
    const res = await rawRequest(port, {
      method: "POST",
      path: "/mcp",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });
    expect(res.status).toBe(401);
    expect(res.headers["www-authenticate"]).toContain("Bearer");
  });

  it("REJECTS the forged admin token end to end", async () => {
    const forged = `x.${b64url({
      team_id: "x",
      client_id: "y",
      role: "admin",
      exp: 9999999999,
    })}.x`;
    const res = await rawRequest(port, {
      method: "POST",
      path: "/mcp",
      headers: { "content-type": "application/json", authorization: `Bearer ${forged}` },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });
    expect(res.status).toBe(401);
  });

  it("accepts a correctly signed token past the auth gate", async () => {
    const res = await rawRequest(port, {
      method: "POST",
      path: "/mcp",
      headers: {
        "content-type": "application/json",
        accept: "application/json, text/event-stream",
        authorization: `Bearer ${signedToken("member")}`,
      },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "ping" }),
    });
    // Past the gate: anything but 401/403 proves the token was accepted and
    // the request reached the MCP handler.
    expect(res.status).not.toBe(401);
    expect(res.status).not.toBe(403);
  });

  it("returns 404 for unknown paths once authenticated", async () => {
    const res = await rawRequest(port, {
      path: "/nope",
      headers: { authorization: `Bearer ${signedToken("member")}` },
    });
    expect(res.status).toBe(404);
  });
});
