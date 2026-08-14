// SPDX-License-Identifier: Apache-2.0
/**
 * Per-request authorization context.
 *
 * The MCP v2 serving entries build a fresh `Server` per request from a
 * zero-argument factory, so verified claims cannot be threaded through as a
 * constructor argument. `AsyncLocalStorage` carries them from the HTTP
 * transport (where the token is verified) to the tool-dispatch path (where the
 * role is enforced) without touching every handler signature.
 *
 * Trust model:
 *   - stdio        → no context at all. The host launched this process; it
 *                    already has local process rights. Nothing to enforce.
 *   - HTTP, no auth→ context with `claims: null, enforced: false`. Only
 *                    reachable when bound to loopback with auth disabled, i.e.
 *                    the same trust boundary as stdio.
 *   - HTTP, auth   → context with verified claims. Roles are enforced.
 */

import { AsyncLocalStorage } from "node:async_hooks";

import { hasPermission, type TeamClaims } from "./auth.js";
import { TOOL_REGISTRY } from "./schema-registry.js";

export interface RequestAuthContext {
  /** Claims verified by `extractAuthClaims`, or null when auth was not required. */
  claims: TeamClaims | null;
  /** True when the transport required authentication for this request. */
  enforced: boolean;
}

const storage = new AsyncLocalStorage<RequestAuthContext>();

/** Run `fn` (and everything it awaits) with the given auth context attached. */
export function runWithAuthContext<T>(ctx: RequestAuthContext, fn: () => T): T {
  return storage.run(ctx, fn);
}

/** The current request's auth context, or undefined on the stdio path. */
export function getAuthContext(): RequestAuthContext | undefined {
  return storage.getStore();
}

/** Thrown when an authenticated caller's role does not permit a tool call. */
export class AuthorizationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AuthorizationError";
  }
}

/**
 * The operation a tool performs, derived from the registry annotation that
 * already declares it. `readOnlyHint: true` means the tool cannot mutate
 * state; everything else is treated as a write.
 *
 * Deliberately coarse: `decision_memory` and `agent_memory` have read-shaped
 * actions too, but they are dispatched through a tool that CAN write, so a
 * viewer is denied the whole tool rather than trusting an `action` argument to
 * decide. Unknown tools fail closed as writes.
 */
export function requiredOperation(toolName: string): "read" | "write" {
  return TOOL_REGISTRY[toolName]?.annotations.readOnlyHint ? "read" : "write";
}

/**
 * Enforce the caller's role against the tool being invoked.
 * Throws `AuthorizationError` when the role is insufficient.
 */
export function assertToolPermission(toolName: string): void {
  const ctx = storage.getStore();
  if (!ctx) return; // stdio — local process, no network identity to enforce

  const operation = requiredOperation(toolName);

  if (!ctx.claims) {
    if (ctx.enforced) {
      // Defensive: the transport rejects unauthenticated requests with 401
      // before dispatch, so reaching here means the gate was bypassed.
      throw new AuthorizationError(
        `Authentication required for '${toolName}' but no verified claims are present`,
      );
    }
    return; // loopback HTTP with auth disabled — same trust boundary as stdio
  }

  if (!hasPermission(ctx.claims.role, operation)) {
    throw new AuthorizationError(
      `Role '${ctx.claims.role}' is not permitted to perform '${operation}' operations (tool: ${toolName})`,
    );
  }
}
