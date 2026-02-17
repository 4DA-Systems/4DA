import { useEffect, useState } from 'react';
import { check } from '@tauri-apps/plugin-updater';

interface UpdateInfo {
  version: string;
  body: string | null;
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
          setUpdate({
            version: result.version,
            body: result.body ?? null,
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
