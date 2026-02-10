import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface DiscoveredContext {
  tech: Array<{ name: string; category: string; confidence: number }>;
  topics: string[];
  lastScan: string | null;
}

export function useContextDiscovery(onStatusChange?: (status: string) => void) {
  const [scanDirectories, setScanDirectories] = useState<string[]>([]);
  const [newScanDir, setNewScanDir] = useState('');
  const [isScanning, setIsScanning] = useState(false);
  const [discoveredContext, setDiscoveredContext] = useState<DiscoveredContext>({
    tech: [],
    topics: [],
    lastScan: null,
  });
  const statusTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const setStatus = useCallback((status: string, duration = 3000) => {
    if (onStatusChange) {
      // Clear previous timeout to prevent accumulation
      if (statusTimeoutRef.current) {
        clearTimeout(statusTimeoutRef.current);
        statusTimeoutRef.current = null;
      }
      onStatusChange(status);
      if (duration > 0) {
        statusTimeoutRef.current = setTimeout(() => {
          onStatusChange('');
          statusTimeoutRef.current = null;
        }, duration);
      }
    }
  }, [onStatusChange]);

  const loadDiscoveredContext = useCallback(async () => {
    try {
      const dirs = await invoke<string[]>('get_context_dirs');
      if (dirs && dirs.length > 0) {
        setScanDirectories(dirs);
      }

      const techResult = await invoke<{
        detected_tech: Array<{ name: string; category: string; confidence: number }>;
      }>('ace_get_detected_tech');

      if (techResult.detected_tech && techResult.detected_tech.length > 0) {
        setDiscoveredContext(prev => ({
          ...prev,
          tech: techResult.detected_tech,
        }));
      }

      const topicsResult = await invoke<{
        topics: Array<{ topic: string; weight: number }>;
      }>('ace_get_active_topics');

      if (topicsResult.topics && topicsResult.topics.length > 0) {
        setDiscoveredContext(prev => ({
          ...prev,
          topics: topicsResult.topics.map(t => t.topic),
        }));
      }
    } catch (error) {
      console.log('No discovered context yet:', error);
    }
  }, []);

  const runAutoDiscovery = useCallback(async () => {
    setIsScanning(true);
    setStatus('Auto-discovering your development context...', 0);

    try {
      const result = await invoke<{
        success: boolean;
        directories_found: number;
        projects_found: number;
        directories_added: number;
        directories: string[];
        scan_result: {
          manifest_scan: { detected_tech: number; confidence: number };
          git_scan: { repos_analyzed: number; total_commits: number };
          combined: { total_topics: number; topics: string[] };
        };
      }>('ace_auto_discover');

      if (result.success) {
        setScanDirectories(result.directories || []);

        const techResult = await invoke<{
          detected_tech: Array<{ name: string; category: string; confidence: number }>;
        }>('ace_get_detected_tech');

        setDiscoveredContext({
          tech: techResult.detected_tech || [],
          topics: result.scan_result?.combined?.topics || [],
          lastScan: new Date().toISOString(),
        });

        setStatus(
          `Auto-discovered ${result.directories_found} dev directories, ${result.projects_found} projects, ${techResult.detected_tech?.length || 0} technologies`,
          5000,
        );
      } else {
        setStatus('No development directories found. Add directories manually below.');
      }
    } catch (error) {
      console.error('Auto-discovery failed:', error);
      setStatus(`Auto-discovery failed: ${error}`);
    } finally {
      setIsScanning(false);
    }
  }, [setStatus]);

  const runFullScan = useCallback(async () => {
    if (scanDirectories.length === 0) {
      return runAutoDiscovery();
    }

    setIsScanning(true);
    setStatus('Scanning directories for context...', 0);

    try {
      const result = await invoke<{
        success: boolean;
        manifest_scan: { detected_tech: number; confidence: number };
        git_scan: { repos_analyzed: number; total_commits: number };
        combined: { total_topics: number; topics: string[] };
      }>('ace_full_scan', { paths: scanDirectories });

      const techResult = await invoke<{
        detected_tech: Array<{ name: string; category: string; confidence: number }>;
      }>('ace_get_detected_tech');

      setDiscoveredContext({
        tech: techResult.detected_tech || [],
        topics: result.combined?.topics || [],
        lastScan: new Date().toISOString(),
      });

      setStatus(
        `Scan complete: ${techResult.detected_tech?.length || 0} technologies, ${result.combined?.total_topics || 0} topics discovered`,
      );
    } catch (error) {
      console.error('Full scan failed:', error);
      setStatus(`Scan failed: ${error}`);
    } finally {
      setIsScanning(false);
    }
  }, [scanDirectories, runAutoDiscovery, setStatus]);

  const addScanDirectory = useCallback(async () => {
    const dirToAdd = newScanDir.trim();
    if (!dirToAdd) {
      setStatus('Please enter a directory path', 2000);
      return;
    }
    if (scanDirectories.includes(dirToAdd)) {
      setStatus('Directory already added', 2000);
      return;
    }

    const newDirs = [...scanDirectories, dirToAdd];

    try {
      await invoke('set_context_dirs', { dirs: newDirs });
      setScanDirectories(newDirs);
      setNewScanDir('');
      setStatus(`Added: ${dirToAdd}`, 2000);
    } catch (error) {
      const errorMsg = String(error).replace('Error: ', '');
      setStatus(`Error: ${errorMsg}`, 4000);
      console.error('Failed to add directory:', error);
    }
  }, [newScanDir, scanDirectories, setStatus]);

  const removeScanDirectory = useCallback(async (dir: string) => {
    const newDirs = scanDirectories.filter(d => d !== dir);
    try {
      await invoke('set_context_dirs', { dirs: newDirs });
      setScanDirectories(newDirs);
      setStatus(`Removed: ${dir}`, 2000);
    } catch (error) {
      const errorMsg = String(error).replace('Error: ', '');
      setStatus(`Error removing: ${errorMsg}`, 3000);
      console.error('Failed to remove directory:', error);
    }
  }, [scanDirectories, setStatus]);

  useEffect(() => {
    loadDiscoveredContext();
    return () => {
      if (statusTimeoutRef.current) clearTimeout(statusTimeoutRef.current);
    };
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
