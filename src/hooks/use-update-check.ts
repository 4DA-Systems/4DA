import { useEffect, useState } from 'react';
import { check } from '@tauri-apps/plugin-updater';

interface UpdateInfo {
  version: string;
  body: string | null;
  canAutoUpdate: boolean;
}

function isLinux(): boolean {
  const ua = navigator.userAgent?.toLowerCase() ?? '';
  if (ua.includes('linux')) return true;
  // Fallback to deprecated navigator.platform for older environments
  return navigator.platform?.toLowerCase().includes('linux') ?? false;
}

export function useUpdateCheck() {
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function checkForUpdate() {
      try {
        const result = await check();
        if (result && !cancelled) {
          // On Linux .deb/.rpm, auto-update can't replace system packages.
          // AppImage auto-update works because it's a self-contained file.
          // We can't reliably detect AppImage vs .deb from the frontend,
          // so on Linux we default to showing a download link. The updater
          // will still attempt auto-update and succeed for AppImage users.
          const canAutoUpdate = !isLinux();

          setUpdate({
            version: result.version,
            body: result.body ?? null,
            canAutoUpdate,
          });
        }
      } catch {
        // Silently ignore update check failures (offline, dev mode, etc.)
      }
    }

    // Delay check by 5s to not block startup
    const timer = setTimeout(checkForUpdate, 5000);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, []);

  const installUpdate = async () => {
    setInstalling(true);
    try {
      const result = await check();
      if (result) {
        await result.downloadAndInstall();
      }
    } catch {
      setInstalling(false);
    }
  };

  const dismiss = () => setUpdate(null);

  return { update, installing, installUpdate, dismiss };
}
