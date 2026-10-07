// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { hasTauriRuntime } from '../lib/tauri-runtime';

/** Build-time version; Vite falls back to '1.0.0' when not launched via an npm script. */
function buildTimeVersion(): string {
  return typeof __APP_VERSION__ === 'string' && __APP_VERSION__ ? __APP_VERSION__ : '';
}

let runtimeVersion: string | null = null;
let pending: Promise<string | null> | null = null;

/** Read the real app version from the Tauri runtime once; shared by every caller. */
export function loadAppVersion(): Promise<string | null> {
  if (runtimeVersion) return Promise.resolve(runtimeVersion);
  if (!hasTauriRuntime()) return Promise.resolve(null);
  pending ??= getVersion()
    .then((v) => {
      runtimeVersion = v || null;
      return runtimeVersion;
    })
    .catch(() => {
      pending = null;
      return null;
    });
  return pending;
}

/** Test hook: forget the cached runtime version. */
export function resetAppVersionCache() {
  runtimeVersion = null;
  pending = null;
}

/**
 * The running app's version. Starts from the build-time constant and
 * replaces it with the Tauri runtime's `getVersion()` (the bundle's real
 * version, from tauri.conf.json). Audit 2026-10-07: the Settings footer read
 * "v1.0.0" on a 1.0.3 app because the build constant had fallen back.
 */
export function useAppVersion(): string {
  const [version, setVersion] = useState(() => runtimeVersion ?? buildTimeVersion());
  useEffect(() => {
    let cancelled = false;
    void loadAppVersion().then((v) => {
      if (!cancelled && v) setVersion(v);
    });
    return () => {
      cancelled = true;
    };
  }, []);
  return version;
}
