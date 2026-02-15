import { useEffect } from 'react';
import { useAppStore } from '../store';

/**
 * Context discovery hook — thin wrapper around Zustand store.
 * All state lives in the store; this hook adds the init-load effect.
 */
export function useContextDiscovery(_onStatusChange?: (status: string) => void) {
  const scanDirectories = useAppStore(s => s.scanDirectories);
  const newScanDir = useAppStore(s => s.newScanDir);
  const setNewScanDir = useAppStore(s => s.setNewScanDir);
  const isScanning = useAppStore(s => s.isScanning);
  const discoveredContext = useAppStore(s => s.discoveredContext);
  const loadDiscoveredContext = useAppStore(s => s.loadDiscoveredContext);
  const runAutoDiscovery = useAppStore(s => s.runAutoDiscovery);
  const runFullScan = useAppStore(s => s.runFullScan);
  const addScanDirectory = useAppStore(s => s.addScanDirectory);
  const removeScanDirectory = useAppStore(s => s.removeScanDirectory);

  useEffect(() => {
    loadDiscoveredContext();
  }, [loadDiscoveredContext]);

  return {
    scanDirectories,
    newScanDir,
    setNewScanDir,
    isScanning,
    discoveredContext,
    loadDiscoveredContext,
    runAutoDiscovery,
    runFullScan,
    addScanDirectory,
    removeScanDirectory,
  };
}
