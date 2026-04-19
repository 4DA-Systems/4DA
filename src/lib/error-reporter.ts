// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

// Error reporter — centralizes console logging for production control
const isDev = import.meta.env.DEV;

export function reportError(context: string, error: unknown): void {
  if (isDev) {
    console.error(`[4DA] ${context}:`, error);
  }
  // Production: silent by default, future: send to telemetry
}

export function reportWarning(context: string, message: string): void {
  if (isDev) {
    console.warn(`[4DA] ${context}:`, message);
  }
}
