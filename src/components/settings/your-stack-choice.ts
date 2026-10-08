// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StackProjectRow } from '../../lib/commands';

/** Is the project inactive (dormant or scratch) and therefore not counted unless forced? */
export function isInactiveProject(p: StackProjectRow): boolean {
  return p.dormant || p.scratch;
}

/**
 * The row after the user's choice — mirrors the backend's
 * `project_inclusion::apply_stack_choice` + `StackMembership::status`.
 * `force` toggles "Force include" (`included` = force on/off) and never
 * changes the include toggle; a plain toggle-off also drops any force.
 */
export function applyStackChoice(p: StackProjectRow, included: boolean, force: boolean): StackProjectRow {
  if (force) {
    return { ...p, forced: included, counts: p.included && (included || !isInactiveProject(p)) };
  }
  const forced = included ? p.forced : false;
  return { ...p, included, forced, counts: included && (forced || !isInactiveProject(p)) };
}
