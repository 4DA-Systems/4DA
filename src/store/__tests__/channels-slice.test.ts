import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('channels-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty channels', () => {
      expect(useAppStore.getState().channels).toEqual([]);
    });

    it('has channelsLoading false', () => {
      expect(useAppStore.getState().channelsLoading).toBe(false);
    });

    it('has activeChannelId null', () => {
      expect(useAppStore.getState().activeChannelId).toBeNull();
    });

    it('has activeRender null', () => {
      expect(useAppStore.getState().activeRender).toBeNull();
    });

    it('has empty activeProvenance', () => {
      expect(useAppStore.getState().activeProvenance).toEqual([]);
    });

    it('has activeChangelog null', () => {
      expect(useAppStore.getState().activeChangelog).toBeNull();
    });

    it('has renderLoading false', () => {
      expect(useAppStore.getState().renderLoading).toBe(false);
    });

    it('has renderError null', () => {
      expect(useAppStore.getState().renderError).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadChannels
  // ---------------------------------------------------------------------------
  describe('loadChannels', () => {
    it('loads channels from backend', async () => {
      const mockChannels = [
        { id: 1, name: 'Rust Updates', description: 'Latest Rust news', source_count: 5 },
        { id: 2, name: 'Web Dev', description: 'Web development', source_count: 8 },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockChannels);

      await useAppStore.getState().loadChannels();

      expect(invoke).toHaveBeenCalledWith('list_channels', {});
      expect(useAppStore.getState().channels).toEqual(mockChannels);
      expect(useAppStore.getState().channelsLoading).toBe(false);
    });

    it('resets loading on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadChannels();

      expect(useAppStore.getState().channelsLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // selectChannel
  // ---------------------------------------------------------------------------
  describe('selectChannel', () => {
    it('selects a channel and loads render with provenance and changelog', async () => {
      const mockRender = { id: 10, channel_id: 1, content: 'Rendered markdown', created_at: '2024-01-01' };
      const mockProvenance = [{ id: 1, render_id: 10, source_url: 'https://example.com' }];
      const mockChangelog = { channel_id: 1, changes: [] };
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockRender)     // get_channel_content
        .mockResolvedValueOnce(mockProvenance)  // get_channel_provenance
        .mockResolvedValueOnce(mockChangelog);  // get_channel_changelog

      await useAppStore.getState().selectChannel(1);

      expect(invoke).toHaveBeenCalledWith('get_channel_content', { channelId: 1 });
      expect(useAppStore.getState().activeChannelId).toBe(1);
      expect(useAppStore.getState().activeRender).toEqual(mockRender);
      expect(useAppStore.getState().renderLoading).toBe(false);
    });

    it('does not load provenance when render is null', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(null); // get_channel_content returns null

      await useAppStore.getState().selectChannel(1);

      expect(useAppStore.getState().activeRender).toBeNull();
      expect(useAppStore.getState().renderLoading).toBe(false);
      // Only one invoke call (get_channel_content), no provenance/changelog
      expect(invoke).toHaveBeenCalledTimes(1);
    });

    it('sets renderError on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Channel not found'));

      await useAppStore.getState().selectChannel(999);

      expect(useAppStore.getState().renderLoading).toBe(false);
      expect(useAppStore.getState().renderError).toContain('Channel not found');
    });
  });

  // ---------------------------------------------------------------------------
  // renderChannel
  // ---------------------------------------------------------------------------
  describe('renderChannel', () => {
    it('renders channel and reloads channels list', async () => {
      const mockRender = { id: 20, channel_id: 1, content: 'Fresh render', created_at: '2024-01-02' };
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockRender)  // render_channel_now
        .mockResolvedValueOnce([])          // list_channels (reload)
        .mockResolvedValueOnce([])          // get_channel_provenance
        .mockResolvedValueOnce(null);       // get_channel_changelog

      await useAppStore.getState().renderChannel(1);

      expect(invoke).toHaveBeenCalledWith('render_channel_now', { channelId: 1 });
      expect(useAppStore.getState().activeRender).toEqual(mockRender);
      expect(useAppStore.getState().renderLoading).toBe(false);
    });

    it('sets renderError on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Render failed'));

      await useAppStore.getState().renderChannel(1);

      expect(useAppStore.getState().renderLoading).toBe(false);
      expect(useAppStore.getState().renderError).toContain('Render failed');
    });
  });

  // ---------------------------------------------------------------------------
  // loadProvenance
  // ---------------------------------------------------------------------------
  describe('loadProvenance', () => {
    it('loads provenance for a render', async () => {
      const mockProv = [{ id: 1, render_id: 10, source_url: 'https://example.com' }];
      vi.mocked(invoke).mockResolvedValueOnce(mockProv);

      await useAppStore.getState().loadProvenance(10);

      expect(invoke).toHaveBeenCalledWith('get_channel_provenance', { renderId: 10 });
      expect(useAppStore.getState().activeProvenance).toEqual(mockProv);
    });

    it('silently handles errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadProvenance(10);

      // No throw, provenance stays empty
      expect(useAppStore.getState().activeProvenance).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // refreshChannelSources
  // ---------------------------------------------------------------------------
  describe('refreshChannelSources', () => {
    it('refreshes sources and reloads channels', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce(5)   // refresh_channel_sources returns count
        .mockResolvedValueOnce([]); // list_channels (reload)

      await useAppStore.getState().refreshChannelSources(1);

      expect(invoke).toHaveBeenCalledWith('refresh_channel_sources', { channelId: 1 });
    });

    it('silently handles errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().refreshChannelSources(1);

      // No throw
    });
  });
});
