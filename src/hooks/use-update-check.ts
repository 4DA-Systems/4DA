// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * The desktop updater (audit 2026-11-11, R5).
 *
 * 4DA lives in the tray, so a process runs for weeks: a check made only once
 * at startup meant most users never heard of a release. It now checks shortly
 * after start, every 6 hours, and when the window regains focus (at most once
 * per 30 minutes). A failed check is recorded where the About panel shows it,
 * instead of being swallowed. Installing shows download progress (the payload
 * is tens of MB), reports a failure in the banner instead of leaving
 * "Installing..." on screen forever, and restarts into the new version through
 * the official process plugin (Windows' NSIS installer restarts the app
 * itself; macOS needs the relaunch).
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { osFamily } from '../lib/platform';
import { registerUpdateCheck, setUpdateStatus } from './update-status';

export const FIRST_CHECK_DELAY_MS = 5_000;
export const UPDATE_RECHECK_MS = 6 * 60 * 60 * 1000;
export const FOCUS_RECHECK_MIN_MS = 30 * 60 * 1000;

export interface UpdateInfo {
  version: string;
  body: string | null;
  canAutoUpdate: boolean;
}

export interface UpdateProgress {
  downloaded: number;
  /** Bytes in the payload, when the server says. */
  total: number | null;
}

/** What went wrong, as text a person can read (plugin errors arrive as strings). */
export function errorText(err: unknown): string {
  if (err instanceof Error && err.message) return err.message;
  if (typeof err === 'string' && err) return err;
  try {
    return JSON.stringify(err) ?? String(err);
  } catch {
    return String(err);
  }
}

export interface InstallFailure {
  /** 'install': nothing changed. 'restart': installed, the app must be restarted by hand. */
  stage: 'install' | 'restart';
  message: string;
}

export function useUpdateCheck() {
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [installError, setInstallError] = useState<InstallFailure | null>(null);

  const handleRef = useRef<Update | null>(null);
  const inFlightRef = useRef<Promise<void> | null>(null);
  const lastAttemptRef = useRef(0);
  const installingRef = useRef(false);
  const dismissedRef = useRef<string | null>(null);

  const runCheck = useCallback((): Promise<void> => {
    if (inFlightRef.current) return inFlightRef.current;
    if (installingRef.current) return Promise.resolve();
    lastAttemptRef.current = Date.now();
    setUpdateStatus({ checking: true });
    const run = (async () => {
      try {
        const result = await check();
        setUpdateStatus({ checking: false, lastCheckedAt: Date.now(), lastError: null });
        if (!result) return;
        const previous = handleRef.current;
        handleRef.current = result;
        if (previous && previous !== result) void previous.close().catch(() => undefined);
        // A dismissed version stays dismissed until a newer one appears.
        if (dismissedRef.current === result.version) return;
        setUpdate({
          version: result.version,
          body: result.body ?? null,
          // A .deb/.rpm cannot be replaced from inside the app, and the
          // frontend cannot tell an AppImage from a package, so Linux gets a
          // link to the release instead of an install button.
          canAutoUpdate: osFamily() !== 'linux',
        });
      } catch (err) {
        setUpdateStatus({ checking: false, lastCheckedAt: Date.now(), lastError: errorText(err) });
      } finally {
        inFlightRef.current = null;
      }
    })();
    inFlightRef.current = run;
    return run;
  }, []);

  useEffect(() => {
    // The updater plugin is registered only in release builds
    // (`#[cfg(not(debug_assertions))]` in lib.rs); DEV mirrors that guard.
    if (import.meta.env.DEV) return;

    const unregister = registerUpdateCheck(runCheck);
    const first = setTimeout(() => void runCheck(), FIRST_CHECK_DELAY_MS);
    const periodic = setInterval(() => void runCheck(), UPDATE_RECHECK_MS);
    const onFocus = () => {
      if (Date.now() - lastAttemptRef.current >= FOCUS_RECHECK_MIN_MS) void runCheck();
    };
    const onVisible = () => {
      if (document.visibilityState === 'visible') onFocus();
    };
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      unregister();
      clearTimeout(first);
      clearInterval(periodic);
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, [runCheck]);

  const installUpdate = useCallback(async () => {
    const handle = handleRef.current;
    if (!handle || installingRef.current) return;
    installingRef.current = true;
    setInstalling(true);
    setInstallError(null);
    setProgress({ downloaded: 0, total: null });

    let downloaded = 0;
    let total: number | null = null;
    let shownPercent = -1;
    let shownBytes = 0;
    const onEvent = (event: DownloadEvent) => {
      if (event.event === 'Started') {
        total = event.data.contentLength ?? null;
        setProgress({ downloaded: 0, total });
      } else if (event.event === 'Progress') {
        downloaded += event.data.chunkLength;
        // One render per percent (or per MB when the size is unknown), not per chunk.
        const percent = total ? Math.floor((downloaded / total) * 100) : -1;
        if (total ? percent !== shownPercent : downloaded - shownBytes >= 1_048_576) {
          shownPercent = percent;
          shownBytes = downloaded;
          setProgress({ downloaded, total });
        }
      } else if (event.event === 'Finished') {
        setProgress({ downloaded, total: total ?? downloaded });
      }
    };

    try {
      try {
        await handle.downloadAndInstall(onEvent);
      } catch (err) {
        setInstallError({ stage: 'install', message: errorText(err) });
        return;
      }
      try {
        await relaunch();
      } catch (err) {
        setInstallError({ stage: 'restart', message: errorText(err) });
      }
    } finally {
      installingRef.current = false;
      setInstalling(false);
      setProgress(null);
    }
  }, []);

  const dismiss = useCallback(() => {
    setUpdate((current) => {
      if (current) dismissedRef.current = current.version;
      return null;
    });
    setInstallError(null);
  }, []);

  return { update, installing, progress, installError, installUpdate, dismiss };
}
