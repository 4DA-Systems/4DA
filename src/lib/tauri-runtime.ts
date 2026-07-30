// SPDX-License-Identifier: FSL-1.1-Apache-2.0

type TauriWindow = Window & {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

export function hasTauriRuntime(): boolean {
  if (typeof window === 'undefined') return false;
  const w = window as TauriWindow;
  return typeof w.__TAURI__ !== 'undefined' || typeof w.__TAURI_INTERNALS__ !== 'undefined';
}

export function isPlainBrowserRuntime(): boolean {
  return typeof window !== 'undefined' && !import.meta.env.VITEST && !hasTauriRuntime();
}
