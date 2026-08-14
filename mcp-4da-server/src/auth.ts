// SPDX-License-Identifier: Apache-2.0
/**
 * MCP Authentication — Bearer token verification for the HTTP transport.
 *
 * Stdio transport (one server process per host, launched by the host itself)
 * is unauthenticated by design: the caller already has local process rights.
 * The HTTP transport verifies every token cryptographically before a single
 * claim is trusted.
 *
 * Verification is HMAC-SHA256 over `base64url(header) + "." + base64url(payload)`
 * compared in constant time against the shared relay secret, followed by
 * expiry, not-before, and role checks. Tokens are issued by the 4DA Team Relay
 * (`relay/src/auth.rs`: `jsonwebtoken` v9, `Header::default()` = HS256,
 * `Validation::default()` = HS256 + mandatory `exp` + 60s leeway) — the
 * parameters below mirror that issuer exactly.
 *
 * FAIL CLOSED: when no shared secret is configured, every token is rejected.
 * A server that cannot verify a signature must never accept the claims inside
 * it — the payload of an unverified JWT is attacker-controlled plaintext.
 */

import { createHmac, timingSafeEqual } from "node:crypto";
import { IncomingMessage } from "node:http";

/** Roles the relay issues. Anything else is rejected. */
export const KNOWN_ROLES = ["admin", "member", "viewer"] as const;
export type TeamRole = (typeof KNOWN_ROLES)[number];

/**
 * The only accepted signing algorithm. Pinning a single algorithm is what
 * closes algorithm-confusion attacks: `alg: none` (no signature at all) and
 * asymmetric algorithms (where a public key can be replayed as the HMAC key)
 * are rejected before any HMAC is computed.
 */
const ACCEPTED_ALG = "HS256";

/** Clock skew tolerance, matching `jsonwebtoken`'s default `leeway` on the issuer. */
const CLOCK_SKEW_SECONDS = 60;

export interface TeamClaims {
  team_id: string;
  client_id: string;
  role: TeamRole;
  exp: number; // Unix timestamp (seconds) — mandatory
}

/**
 * The shared secret used to verify relay-issued tokens.
 *
 * `MCP_AUTH_SECRET` is the name to use when the MCP server is deployed on its
 * own; `JWT_SECRET` is accepted as a fallback so a server co-deployed with the
 * relay (which reads `JWT_SECRET`) inherits the same value without duplicating
 * the config. Whitespace-only values count as unset.
 */
export function getAuthSecret(): string | undefined {
  const secret =
    process.env.MCP_AUTH_SECRET?.trim() || process.env.JWT_SECRET?.trim();
  return secret ? secret : undefined;
}

/** Constant-time buffer comparison that tolerates length mismatch. */
function constantTimeEquals(a: Buffer, b: Buffer): boolean {
  // timingSafeEqual throws on differing lengths. A wrong-length signature is
  // unconditionally invalid, so returning early leaks nothing an attacker
  // cannot already compute (HS256 signatures are always 32 bytes).
  if (a.length !== b.length) return false;
  return timingSafeEqual(a, b);
}

function decodeSegment(segment: string): unknown {
  const json = Buffer.from(segment, "base64url").toString("utf-8");
  return JSON.parse(json) as unknown;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

/** Reject anything that is not a finite number (NaN, Infinity, strings, null). */
function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function reject(reason: string): null {
  // console.warn writes to stderr, which is the only stream an MCP stdio
  // server may write diagnostics to. Never log the token itself.
  console.warn(`[4DA MCP Auth] Token rejected: ${reason}`);
  return null;
}

/**
 * Verify a JWT's signature and claims. Returns the claims only when the
 * signature checks out against the configured secret AND the claims are valid.
 *
 * @param token   The raw JWT (no "Bearer " prefix).
 * @param nowMs   Current time in epoch milliseconds (injectable for tests).
 */
export function verifyToken(token: string, nowMs: number = Date.now()): TeamClaims | null {
  const secret = getAuthSecret();
  if (!secret) {
    // Fail closed. Without a secret there is nothing to verify against, so no
    // token can be trusted — including one that is perfectly well-formed.
    return reject(
      "no shared secret configured (set MCP_AUTH_SECRET). Refusing to trust unverifiable tokens",
    );
  }

  const parts = token.split(".");
  if (parts.length !== 3 || parts.some((p) => p.length === 0)) {
    return reject("malformed token (expected three non-empty segments)");
  }
  const [encodedHeader, encodedPayload, encodedSignature] = parts;

  // --- 1. Header: pin the algorithm BEFORE doing any crypto ---------------
  let header: unknown;
  try {
    header = decodeSegment(encodedHeader);
  } catch {
    return reject("undecodable header segment");
  }
  if (!isPlainObject(header)) return reject("header is not a JSON object");
  if (header.alg !== ACCEPTED_ALG) {
    return reject(`unsupported alg '${String(header.alg)}' (only ${ACCEPTED_ALG} is accepted)`);
  }
  if (header.typ !== undefined && header.typ !== "JWT") {
    return reject(`unsupported typ '${String(header.typ)}'`);
  }
  if (header.crit !== undefined) {
    // `crit` marks header params a verifier MUST understand. We understand
    // none of them, so the only spec-correct response is to refuse.
    return reject("token declares critical header params this verifier does not implement");
  }

  // --- 2. Signature: constant-time HMAC-SHA256 over header.payload -------
  const signingInput = `${encodedHeader}.${encodedPayload}`;
  const expected = createHmac("sha256", secret).update(signingInput).digest();
  const provided = Buffer.from(encodedSignature, "base64url");
  if (!constantTimeEquals(expected, provided)) {
    return reject("signature verification failed");
  }

  // --- 3. Claims: only now is the payload trustworthy --------------------
  let payload: unknown;
  try {
    payload = decodeSegment(encodedPayload);
  } catch {
    return reject("undecodable payload segment");
  }
  if (!isPlainObject(payload)) return reject("payload is not a JSON object");

  const { team_id, client_id, role, exp, nbf } = payload;

  if (!isNonEmptyString(team_id)) return reject("missing or invalid team_id");
  if (!isNonEmptyString(client_id)) return reject("missing or invalid client_id");
  if (!isNonEmptyString(role)) return reject("missing or invalid role");
  if (!(KNOWN_ROLES as readonly string[]).includes(role)) {
    return reject(`unknown role '${role}'`);
  }

  // `exp` is mandatory: a signed token with no expiry is a permanent
  // credential, and the issuer always sets one.
  if (!isFiniteNumber(exp)) return reject("missing or invalid exp claim");
  const nowSeconds = Math.floor(nowMs / 1000);
  if (exp + CLOCK_SKEW_SECONDS < nowSeconds) {
    return reject(`token expired for team ${team_id}`);
  }
  if (nbf !== undefined) {
    if (!isFiniteNumber(nbf)) return reject("invalid nbf claim");
    if (nbf - CLOCK_SKEW_SECONDS > nowSeconds) return reject("token not yet valid (nbf)");
  }

  return { team_id, client_id, role: role as TeamRole, exp };
}

/**
 * Extract and verify the Bearer token on an HTTP request.
 *
 * Returns the verified claims, or null when the request carries no token, a
 * malformed token, a forged/tampered token, or an expired one.
 */
export function extractAuthClaims(req: IncomingMessage): TeamClaims | null {
  const auth = req.headers["authorization"];
  if (typeof auth !== "string" || !auth.startsWith("Bearer ")) {
    return null;
  }

  const token = auth.slice(7).trim();
  if (!token) return null;

  return verifyToken(token);
}

/**
 * Check if a role has permission for a specific operation.
 * Viewers can read, members can read+write, admins can do everything.
 *
 * Enforced on the tool-dispatch path — see `assertToolPermission` in
 * `auth-context.ts`, which maps each tool to an operation via its
 * `readOnlyHint` annotation.
 */
export function hasPermission(
  role: string,
  operation: "read" | "write" | "admin",
): boolean {
  switch (operation) {
    case "read":
      return (KNOWN_ROLES as readonly string[]).includes(role);
    case "write":
      return role === "member" || role === "admin";
    case "admin":
      return role === "admin";
    default:
      return false;
  }
}
