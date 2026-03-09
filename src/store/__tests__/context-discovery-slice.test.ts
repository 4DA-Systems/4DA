import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('context-discovery-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty scanDirectories', () => {
      expect(useAppStore.getState().scanDirectories).toEqual([]);
    });

    it('has empty newScanDir', () => {
      expect(useAppStore.getState().newScanDir).toBe('');
    });

    it('has isScanning false', () => {
      expect(useAppStore.getState().isScanning).toBe(false);
    });

    it('has discoveredContext with defaults', () => {
      const ctx = useAppStore.getState().discoveredContext;
      expect(ctx.tech).toEqual([]);
      expect(ctx.topics).toEqual([]);
      expect(ctx.lastScan).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // setNewScanDir
  // ---------------------------------------------------------------------------
  describe('setNewScanDir', () => {
    it('updates newScanDir', () => {
      useAppStore.getState().setNewScanDir('/home/user/projects');

      expect(useAppStore.getState().newScanDir).toBe('/home/user/projects');
    });
  });

  // ---------------------------------------------------------------------------
  // loadDiscoveredContext
  // ---------------------------------------------------------------------------
  describe('loadDiscoveredContext', () => {
    it('loads directories, tech, and topics', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce(['/home/user/project1', '/home/user/project2']) // get_context_dirs
        .mockResolvedValueOnce({ detected_tech: [{ name: 'React', category: 'frontend', confidence: 0.95 }] }) // ace_get_detected_tech
        .mockResolvedValueOnce({ topics: [{ topic: 'web-dev', weight: 0.8 }] }); // ace_get_active_topics

      await useAppStore.getState().loadDiscoveredContext();

      expect(invoke).toHaveBeenCalledWith('get_context_dirs', {});
      expect(invoke).toHaveBeenCalledWith('ace_get_detected_tech', {});
      expect(invoke).toHaveBeenCalledWith('ace_get_active_topics', {});
      expect(useAppStore.getState().scanDirectories).toEqual(['/home/user/project1', '/home/user/project2']);
      expect(useAppStore.getState().discoveredContext.tech).toHaveLength(1);
      expect(useAppStore.getState().discoveredContext.topics).toEqual(['web-dev']);
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadDiscoveredContext();

      // Should not throw, state remains at defaults
      expect(useAppStore.getState().scanDirectories).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // addScanDirectory
  // ---------------------------------------------------------------------------
  describe('addScanDirectory', () => {
    it('adds a new directory', async () => {
      useAppStore.setState({ newScanDir: '/new/path' });
      vi.mocked(invoke).mockResolvedValueOnce(undefined); // set_context_dirs

      await useAppStore.getState().addScanDirectory();

      expect(invoke).toHaveBeenCalledWith('set_context_dirs', { dirs: ['/new/path'] });
      expect(useAppStore.getState().scanDirectories).toContain('/new/path');
      expect(useAppStore.getState().newScanDir).toBe('');
    });

    it('does not add empty directory', async () => {
      useAppStore.setState({ newScanDir: '   ' });

      await useAppStore.getState().addScanDirectory();

      expect(invoke).not.toHaveBeenCalled();
    });

    it('does not add duplicate directory', async () => {
      useAppStore.setState({
        newScanDir: '/existing',
        scanDirectories: ['/existing'],
      });

      await useAppStore.getState().addScanDirectory();

      expect(invoke).not.toHaveBeenCalled();
    });
  });

  // ---------------------------------------------------------------------------
  // removeScanDirectory
  // ---------------------------------------------------------------------------
  describe('removeScanDirectory', () => {
    it('removes a directory from the list', async () => {
      useAppStore.setState({ scanDirectories: ['/a', '/b', '/c'] });
      vi.mocked(invoke).mockResolvedValueOnce(undefined); // set_context_dirs

      await useAppStore.getState().removeScanDirectory('/b');

      expect(invoke).toHaveBeenCalledWith('set_context_dirs', { dirs: ['/a', '/c'] });
      expect(useAppStore.getState().scanDirectories).toEqual(['/a', '/c']);
    });

    it('handles errors gracefully', async () => {
      useAppStore.setState({ scanDirectories: ['/a'] });
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().removeScanDirectory('/a');

      // Should not throw
    });
  });

  // ---------------------------------------------------------------------------
  // runAutoDiscovery
  // ---------------------------------------------------------------------------
  describe('runAutoDiscovery', () => {
    it('sets isScanning during operation', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce({
          success: true,
          directories_found: 2,
          projects_found: 3,
          directories_added: 2,
          directories: ['/home/dev1', '/home/dev2'],
          scan_result: { manifest_scan: { detected_tech: 5, confidence: 0.9 }, git_scan: { repos_analyzed: 2, total_commits: 100 }, combined: { total_topics: 3, topics: ['rust', 'react', 'tauri'] } },
        }) // ace_auto_discover
        .mockResolvedValueOnce({ detected_tech: [{ name: 'Rust', category: 'language', confidence: 0.99 }] }); // ace_get_detected_tech

      await useAppStore.getState().runAutoDiscovery();

      expect(useAppStore.getState().isScanning).toBe(false);
      expect(useAppStore.getState().scanDirectories).toEqual(['/home/dev1', '/home/dev2']);
      expect(useAppStore.getState().discoveredContext.tech).toHaveLength(1);
      expect(useAppStore.getState().discoveredContext.topics).toEqual(['rust', 'react', 'tauri']);
    });

    it('handles unsuccessful result', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({ success: false });

      await useAppStore.getState().runAutoDiscovery();

      expect(useAppStore.getState().isScanning).toBe(false);
    });

    it('handles errors and resets scanning', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().runAutoDiscovery();

      expect(useAppStore.getState().isScanning).toBe(false);
    });
  });
});
