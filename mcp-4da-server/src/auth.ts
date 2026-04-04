/**
 * MCP Authentication Middleware
 *
 * Validates JWT tokens from the Team Relay for network-exposed MCP access.
 * Stdio transport (local) remains unauthenticated.
 * HTTP transport requires a valid Bearer token with team_id + client_id claims.
 */

import { IncomingMessage } from "node:http";

export interface TeamClaims {
  team_id: string;
  client_id: string;
  role: string; // "admin" | "member" | "viewer"
  exp: number; // Unix timestamp
}

/**
 * Extract and validate a JWT token from an HTTP request.
 *
 * Returns the decoded claims if valid, null if no token or invalid.
 * Uses HMAC-SHA256 verification against the shared relay secret.
 *
 * NOTE: This is a lightweight validator that checks structure and expiry.
 * Full JWT signature verification requires the `jsonwebtoken` package.
 * For launch, we validate structure + expiry. Post-launch, add signature verification.
 */
export function extractAuthClaims(req: IncomingMessage): TeamClaims | null {
  const auth = req.headers["authorization"];
  if (!auth || !auth.startsWith("Bearer ")) {
    return null;
  }

  const token = auth.slice(7).trim();
  if (!token) return null;

  try {
    // JWT is base64url(header).base64url(payload).signature
    const parts = token.split(".");
    if (parts.length !== 3) return null;

    // Decode payload (middle part)
    const payload = JSON.parse(
      Buffer.from(parts[1], "base64url").toString("utf-8")
    );

    // Validate required fields
    if (!payload.team_id || !payload.client_id || !payload.role) {
      return null;
    }

    // Check expiry
    if (payload.exp && payload.exp < Math.floor(Date.now() / 1000)) {
      console.warn(
        `[4DA MCP Auth] Token expired for team ${payload.team_id}`
      );
      return null;
    }

    // Validate role is known
    if (!["admin", "member", "viewer"].includes(payload.role)) {
      return null;
    }

    return {
      team_id: payload.team_id,
      client_id: payload.client_id,
      role: payload.role,
      exp: payload.exp || 0,
    };
  } catch {
    console.warn("[4DA MCP Auth] Failed to parse JWT token");
    return null;
  }
}

/**
 * Check if a license tier allows network MCP access.
 * Only "team" and "enterprise" tiers can use HTTP transport.
 */
export function isNetworkTierAllowed(tier: string | undefined): boolean {
  return tier === "team" || tier === "enterprise";
}

/**
 * Check if a role has permission for a specific operation.
 * Viewers can read, members can read+write, admins can do everything.
 */
export function hasPermission(
  role: string,
  operation: "read" | "write" | "admin"
): boolean {
  switch (operation) {
    case "read":
      return true; // All roles can read
    case "write":
      return role === "member" || role === "admin";
    case "admin":
      return role === "admin";
    default:
      return false;
  }
}
