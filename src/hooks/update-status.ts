// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * The outcome of the app's last update check, shared between the one
 * scheduler (`useUpdateCheck`, mounted once in App) and the About panel, which
 * shows it. A failed check used to be swallowed, so an install that could
 * never reach the update server looked exactly like one that was up to date.
 */
import { useSyncExternalStore } from 'react';

export interface UpdateCheckStatus {
  checking: boolean;
  /** Epoch ms of the last check that finished, success or failure. */
  lastCheckedAt: number | null;
  /** Why the last check failed; null after a successful check. */
  lastError: string | null;
}

const INITIAL: UpdateCheckStatus = { checking: false, lastCheckedAt: null, lastError: null };

let status: UpdateCheckStatus = INITIAL;
const listeners = new Set<() => void>();
let runner: (() => Promise<void>) | null = null;

export function getUpdateStatus(): UpdateCheckStatus {
  return status;
}

export function setUpdateStatus(patch: Partial<UpdateCheckStatus>): void {
  status = { ...status, ...patch };
  listeners.forEach((l) => l());
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The scheduler registers its check so "Check now" runs the same code path. */
export function registerUpdateCheck(fn: () => Promise<void>): () => void {
  runner = fn;
  return () => {
    if (runner === fn) runner = null;
  };
}

/** Run a check now. False when no scheduler is mounted (a dev build). */
export function canCheckForUpdates(): boolean {
  return runner !== null;
}

export function requestUpdateCheck(): Promise<void> {
  return runner ? runner() : Promise.resolve();
}

export function useUpdateStatus(): UpdateCheckStatus {
  return useSyncExternalStore(subscribe, getUpdateStatus);
}

/** Test-only: forget status and the registered scheduler. */
export function __resetUpdateStatusForTests(): void {
  status = INITIAL;
  runner = null;
  listeners.clear();
}
