// SPDX-License-Identifier: Apache-2.0
/**
 * Security regression tests for MCP Bearer-token authentication.
 *
 * Origin: through v5.0.0 `extractAuthClaims` split the JWT, base64-decoded the
 * payload, validated the CLAIMS, and returned them — never reading the
 * signature segment and never computing an HMAC. Anyone could mint
 * `Bearer x.<base64url({"role":"admin",...})>.x` and be an admin on a
 * network-exposed `--http` server.
 *
 * The first test in this file is the proof: it constructs exactly that token
 * and asserts it is refused. It fails against the pre-fix implementation
 * (which returns the forged claims) and passes only when the signature is
 * actually verified.
 *
 * The rest lock in the surrounding contract: fail-closed with no secret,
 * expiry enforcement, algorithm pinning (no `alg: none`, no algorithm
 * confusion), tamper detection, and role enforcement on the dispatch path.
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { createHmac } from "node:crypto";
import type { IncomingMessage } from "node:http";

import {
  extractAuthClaims,
  getAuthSecret,
  hasPermission,
  verifyToken,
  type TeamClaims,
  type TeamRole,
} from "../auth.js";
import {
  assertToolPermission,
  AuthorizationError,
  requiredOperation,
  runWithAuthContext,
} from "../auth-context.js";
import { TOOL_REGISTRY } from "../schema-registry.js";

const SECRET = "relay-shared-secret-for-tests";
const FAR_FUTURE = 9999999999;

const b64url = (value: unknown): string =>
  Buffer.from(JSON.stringify(value)).toString("base64url");

/** Mint a genuinely signed token. */
function sign(
  payload: Record<string, unknown>,
  secret: string = SECRET,
  header: Record<string, unknown> = { alg: "HS256", typ: "JWT" },
): string {
  const signingInput = `${b64url(header)}.${b64url(payload)}`;
  const signature = createHmac("sha256", secret).update(signingInput).digest("base64url");
  return `${signingInput}.${signature}`;
}

/** Mint a token with a garbage signature — the original exploit shape. */
function forge(
  payload: Record<string, unknown>,
  header: Record<string, unknown> = { alg: "HS256", typ: "JWT" },
): string {
  return `${b64url(header)}.${b64url(payload)}.x`;
}

const adminPayload = {
  team_id: "attacker-team",
  client_id: "attacker-client",
  role: "admin",
  exp: FAR_FUTURE,
};

const validPayload = {
  team_id: "team-alpha",
  client_id: "client-1",
  role: "member",
  exp: FAR_FUTURE,
};

function requestWith(headers: Record<string, string | undefined>): IncomingMessage {
  return { headers } as unknown as IncomingMessage;
}

// Rejections log to stderr via console.warn; capture instead of spamming the
// test output, and assert the log never carries the token itself.
let warnings: string[] = [];
const realWarn = console.warn;
const savedEnv = {
  MCP_AUTH_SECRET: process.env.MCP_AUTH_SECRET,
  JWT_SECRET: process.env.JWT_SECRET,
};

beforeEach(() => {
  process.env.MCP_AUTH_SECRET = SECRET;
  delete process.env.JWT_SECRET;
  warnings = [];
  console.warn = (...args: unknown[]) => {
    warnings.push(args.map((a) => String(a)).join(" "));
  };
});

afterEach(() => {
  console.warn = realWarn;
  for (const [key, value] of Object.entries(savedEnv)) {
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
});

describe("forged tokens (the v5.0.0 flaw)", () => {
  it("REJECTS the exploit token: Bearer x.<base64url(admin claims)>.x", () => {
    const exploit = `x.${b64url(adminPayload)}.x`;
    const claims = extractAuthClaims(
      requestWith({ authorization: `Bearer ${exploit}` }),
    );
    expect(claims).toBeNull();
  });

  it("REJECTS a well-formed header with an unsigned admin payload", () => {
    expect(verifyToken(forge(adminPayload))).toBeNull();
  });

  it("REJECTS a token signed with the wrong secret", () => {
    expect(verifyToken(sign(validPayload, "some-other-secret"))).toBeNull();
  });

  it("REJECTS a token whose payload was swapped after signing (privilege escalation)", () => {
    const legit = sign({ ...validPayload, role: "viewer" });
    const [header, , signature] = legit.split(".");
    const escalated = `${header}.${b64url({ ...validPayload, role: "admin" })}.${signature}`;
    expect(verifyToken(escalated)).toBeNull();
  });

  it("never writes the rejected token to the log", () => {
    verifyToken(forge(adminPayload));
    expect(warnings.length).toBeGreaterThan(0);
    expect(warnings.join(" ")).not.toContain(b64url(adminPayload));
  });
});

describe("valid tokens", () => {
  it("ACCEPTS a correctly signed, unexpired token and returns its claims", () => {
    const claims: TeamClaims | null = verifyToken(sign(validPayload));
    expect(claims).toEqual({
      team_id: "team-alpha",
      client_id: "client-1",
      role: "member",
      exp: FAR_FUTURE,
    });
  });

  it("ACCEPTS a signed token through the Authorization header", () => {
    const claims = extractAuthClaims(
      requestWith({ authorization: `Bearer ${sign(validPayload)}` }),
    );
    expect(claims?.role).toBe("member");
  });

  it("tolerates the issuer's 60s clock skew on exp", () => {
    const nowMs = 1_800_000_000_000;
    const justExpired = { ...validPayload, exp: Math.floor(nowMs / 1000) - 30 };
    expect(verifyToken(sign(justExpired), nowMs)).not.toBeNull();
  });
});

describe("expiry", () => {
  it("REJECTS a validly signed but expired token", () => {
    const expired = { ...validPayload, exp: Math.floor(Date.now() / 1000) - 3600 };
    expect(verifyToken(sign(expired))).toBeNull();
  });

  it("REJECTS a validly signed token with no exp claim (would never expire)", () => {
    const { exp: _exp, ...noExp } = validPayload;
    expect(verifyToken(sign(noExp))).toBeNull();
  });

  it("REJECTS a token that is not yet valid (nbf in the future)", () => {
    const nowMs = 1_800_000_000_000;
    const notYet = { ...validPayload, nbf: Math.floor(nowMs / 1000) + 600 };
    expect(verifyToken(sign(notYet), nowMs)).toBeNull();
  });
});

describe("fail-closed without a configured secret", () => {
  beforeEach(() => {
    delete process.env.MCP_AUTH_SECRET;
    delete process.env.JWT_SECRET;
  });

  it("reports no secret", () => {
    expect(getAuthSecret()).toBeUndefined();
  });

  it("REJECTS every token, including well-formed signed ones", () => {
    expect(verifyToken(sign(validPayload))).toBeNull();
    expect(verifyToken(forge(adminPayload))).toBeNull();
    expect(
      extractAuthClaims(requestWith({ authorization: `Bearer ${sign(validPayload)}` })),
    ).toBeNull();
  });

  it("treats a whitespace-only secret as unset", () => {
    process.env.MCP_AUTH_SECRET = "   ";
    expect(getAuthSecret()).toBeUndefined();
    expect(verifyToken(sign(validPayload, "   "))).toBeNull();
  });

  it("falls back to JWT_SECRET so a relay-co-deployed server shares one value", () => {
    process.env.JWT_SECRET = SECRET;
    expect(getAuthSecret()).toBe(SECRET);
    expect(verifyToken(sign(validPayload))).not.toBeNull();
  });
});

describe("algorithm pinning", () => {
  it("REJECTS alg: none", () => {
    expect(verifyToken(forge(adminPayload, { alg: "none", typ: "JWT" }))).toBeNull();
    // ...including the canonical empty-signature form
    expect(verifyToken(`${b64url({ alg: "none" })}.${b64url(adminPayload)}.`)).toBeNull();
  });

  it("REJECTS other HMAC variants (HS384/HS512) even when correctly signed", () => {
    for (const alg of ["HS384", "HS512"]) {
      const header = { alg, typ: "JWT" };
      const signingInput = `${b64url(header)}.${b64url(adminPayload)}`;
      const sig = createHmac(alg === "HS384" ? "sha384" : "sha512", SECRET)
        .update(signingInput)
        .digest("base64url");
      expect(verifyToken(`${signingInput}.${sig}`), alg).toBeNull();
    }
  });

  it("REJECTS asymmetric algs (RS256 confusion attempt)", () => {
    expect(verifyToken(sign(adminPayload, SECRET, { alg: "RS256", typ: "JWT" }))).toBeNull();
  });

  it("REJECTS unknown critical header params", () => {
    expect(
      verifyToken(sign(adminPayload, SECRET, { alg: "HS256", typ: "JWT", crit: ["exp"] })),
    ).toBeNull();
  });

  it("REJECTS a non-JWT typ", () => {
    expect(verifyToken(sign(adminPayload, SECRET, { alg: "HS256", typ: "JWE" }))).toBeNull();
  });
});

describe("claim validation (after signature verification)", () => {
  it("REJECTS signed tokens missing required claims", () => {
    expect(verifyToken(sign({ client_id: "c", role: "admin", exp: FAR_FUTURE }))).toBeNull();
    expect(verifyToken(sign({ team_id: "t", role: "admin", exp: FAR_FUTURE }))).toBeNull();
    expect(verifyToken(sign({ team_id: "t", client_id: "c", exp: FAR_FUTURE }))).toBeNull();
  });

  it("REJECTS an unknown role even when signed", () => {
    expect(verifyToken(sign({ ...validPayload, role: "superuser" }))).toBeNull();
  });

  it("REJECTS malformed tokens", () => {
    expect(verifyToken("")).toBeNull();
    expect(verifyToken("not-a-jwt")).toBeNull();
    expect(verifyToken("a.b")).toBeNull();
    expect(verifyToken("a.b.c.d")).toBeNull();
    expect(verifyToken(`${b64url({ alg: "HS256" })}..sig`)).toBeNull();
  });
});

describe("Authorization header handling", () => {
  it("returns null when there is no Authorization header", () => {
    expect(extractAuthClaims(requestWith({}))).toBeNull();
  });

  it("returns null for non-Bearer schemes", () => {
    expect(extractAuthClaims(requestWith({ authorization: "Basic dXNlcjpwYXNz" }))).toBeNull();
  });

  it("returns null for an empty Bearer value", () => {
    expect(extractAuthClaims(requestWith({ authorization: "Bearer    " }))).toBeNull();
  });
});

describe("hasPermission", () => {
  it("lets every known role read", () => {
    for (const role of ["viewer", "member", "admin"]) {
      expect(hasPermission(role, "read"), role).toBe(true);
    }
  });

  it("lets only members and admins write", () => {
    expect(hasPermission("viewer", "write")).toBe(false);
    expect(hasPermission("member", "write")).toBe(true);
    expect(hasPermission("admin", "write")).toBe(true);
  });

  it("lets only admins perform admin operations", () => {
    expect(hasPermission("viewer", "admin")).toBe(false);
    expect(hasPermission("member", "admin")).toBe(false);
    expect(hasPermission("admin", "admin")).toBe(true);
  });

  it("denies unknown roles everything", () => {
    expect(hasPermission("superuser", "read")).toBe(false);
    expect(hasPermission("superuser", "write")).toBe(false);
  });
});

describe("role enforcement on the dispatch path", () => {
  const claims = (role: string): TeamClaims => ({
    team_id: "t",
    client_id: "c",
    role: role as TeamRole,
    exp: FAR_FUTURE,
  });

  const WRITE_TOOLS = ["record_feedback", "decision_memory", "agent_memory"];
  const READ_TOOLS = ["vulnerability_scan", "get_context", "developer_dna"];

  it("derives each tool's operation from its readOnlyHint annotation", () => {
    for (const [name, entry] of Object.entries(TOOL_REGISTRY)) {
      expect(requiredOperation(name), name).toBe(entry.annotations.readOnlyHint ? "read" : "write");
    }
    for (const tool of WRITE_TOOLS) expect(requiredOperation(tool), tool).toBe("write");
    for (const tool of READ_TOOLS) expect(requiredOperation(tool), tool).toBe("read");
  });

  it("fails closed for unknown tools", () => {
    expect(requiredOperation("no_such_tool")).toBe("write");
  });

  it("does not enforce on the stdio path (no auth context)", () => {
    for (const tool of [...READ_TOOLS, ...WRITE_TOOLS]) {
      expect(() => assertToolPermission(tool)).not.toThrow();
    }
  });

  it("BLOCKS a viewer from every write tool", () => {
    runWithAuthContext({ claims: claims("viewer"), enforced: true }, () => {
      for (const tool of WRITE_TOOLS) {
        expect(() => assertToolPermission(tool), tool).toThrow(AuthorizationError);
      }
    });
  });

  it("allows a viewer to read", () => {
    runWithAuthContext({ claims: claims("viewer"), enforced: true }, () => {
      for (const tool of READ_TOOLS) {
        expect(() => assertToolPermission(tool), tool).not.toThrow();
      }
    });
  });

  it("allows members and admins to write", () => {
    for (const role of ["member", "admin"]) {
      runWithAuthContext({ claims: claims(role), enforced: true }, () => {
        for (const tool of WRITE_TOOLS) {
          expect(() => assertToolPermission(tool), `${role}/${tool}`).not.toThrow();
        }
      });
    }
  });

  it("refuses dispatch when auth was required but no claims reached the context", () => {
    runWithAuthContext({ claims: null, enforced: true }, () => {
      expect(() => assertToolPermission("get_context")).toThrow(AuthorizationError);
    });
  });

  it("allows unauthenticated loopback HTTP (same trust boundary as stdio)", () => {
    runWithAuthContext({ claims: null, enforced: false }, () => {
      expect(() => assertToolPermission("record_feedback")).not.toThrow();
    });
  });
});
