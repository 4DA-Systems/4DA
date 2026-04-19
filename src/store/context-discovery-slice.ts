// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore, ContextDiscoverySlice } from './types';

export const createContextDiscoverySlice: StateCreator<AppStore, [], [], ContextDiscoverySlice> = (set, get) => ({
  scanDirectories: [],
  newScanDir: '',
  isScanning: false,
  discoveredContext: { tech: [], topics: [], lastScan: null },

  setNewScanDir: (dir) => set({ newScanDir: dir }),

  loadDiscoveredContext: async () => {
    const [dirsResult, techResult, topicsResult] = await Promise.allSettled([
      cmd('get_context_dirs'),
      cmd('ace_get_detected_tech'),
      cmd('ace_get_active_topics'),
    ]);

    if (dirsResult.status === 'fulfilled' && dirsResult.value && dirsResult.value.length > 0) {
      set({ scanDirectories: dirsResult.value });
    }

    if (techResult.status === 'fulfilled' && techResult.value?.detected_tech?.length > 0) {
      set(state => ({
        discoveredContext: { ...state.discoveredContext, tech: techResult.value.detected_tech },
      }));
    }

    if (topicsResult.status === 'fulfilled' && topicsResult.value?.topics?.length > 0) {
      set(state => ({
        discoveredContext: {
          ...state.discoveredContext,
          topics: topicsResult.value.topics.map(t => t.topic),
        },
      }));
    }
  },

  runAutoDiscovery: async () => {
    const { setSettingsStatus } = get();
    set({ isScanning: true });
    setSettingsStatus('Auto-discovering your development context...');

    try {
      const result = await cmd('ace_auto_discover');

      if (result.success) {
        set({ scanDirectories: result.directories || [] });

        const techResult = await cmd('ace_get_detected_tech');

        set({
          discoveredContext: {
            tech: techResult.detected_tech || [],
            topics: result.scan_result?.combined?.topics || [],
            lastScan: new Date().toISOString(),
          },
        });

        setSettingsStatus(
          `Auto-discovered ${result.directories_found} dev directories, ${result.projects_found} projects, ${techResult.detected_tech?.length || 0} technologies`,
        );
        setTimeout(() => set({ settingsStatus: '' }), 5000);
      } else {
        setSettingsStatus('No development directories found. Add directories manually below.');
        setTimeout(() => set({ settingsStatus: '' }), 3000);
      }
    } catch (error) {
      console.error('Auto-discovery failed:', error);
      setSettingsStatus(`Auto-discovery failed: ${error}`);
    } finally {
      set({ isScanning: false });
    }
  },

  runFullScan: async () => {
    const { scanDirectories, runAutoDiscovery, setSettingsStatus } = get();

    if (scanDirectories.length === 0) {
      return runAutoDiscovery();
    }

    set({ isScanning: true });
    setSettingsStatus('Scanning directories for context...');

    try {
      const result = await cmd('ace_full_scan', { paths: scanDirectories });

      const techResult = await cmd('ace_get_detected_tech');

      set({
        discoveredContext: {
          tech: techResult.detected_tech || [],
          topics: result.combined?.topics || [],
          lastScan: new Date().toISOString(),
        },
      });

      setSettingsStatus(
        `Scan complete: ${techResult.detected_tech?.length || 0} technologies, ${result.combined?.total_topics || 0} topics discovered`,
      );
      setTimeout(() => set({ settingsStatus: '' }), 3000);
    } catch (error) {
      console.error('Full scan failed:', error);
      setSettingsStatus(`Scan failed: ${error}`);
    } finally {
      set({ isScanning: false });
    }
  },

  addScanDirectory: async () => {
    const { newScanDir, scanDirectories, setSettingsStatus } = get();
    const dirToAdd = newScanDir.trim();
    if (!dirToAdd) {
      setSettingsStatus('Please enter a directory path');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
      return;
    }
    if (scanDirectories.includes(dirToAdd)) {
      setSettingsStatus('Directory already added');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
      return;
    }

    const newDirs = [...scanDirectories, dirToAdd];

    try {
      await cmd('set_context_dirs', { dirs: newDirs });
      set({ scanDirectories: newDirs, newScanDir: '' });
      setSettingsStatus(`Added: ${dirToAdd}`);
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      const errorMsg = String(error).replace('Error: ', '');
      setSettingsStatus(`Error: ${errorMsg}`);
      console.error('Failed to add directory:', error);
    }
  },

  removeScanDirectory: async (dir) => {
    const { scanDirectories, setSettingsStatus } = get();
    const newDirs = scanDirectories.filter(d => d !== dir);
    try {
      await cmd('set_context_dirs', { dirs: newDirs });
      set({ scanDirectories: newDirs });
      setSettingsStatus(`Removed: ${dir}`);
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      const errorMsg = String(error).replace('Error: ', '');
      setSettingsStatus(`Error removing: ${errorMsg}`);
      console.error('Failed to remove directory:', error);
    }
  },
});
