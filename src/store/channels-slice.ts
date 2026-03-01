import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore } from './types';
import type { ChannelSummary, ChannelRender, RenderProvenance, ChannelChangelog } from '../types/channels';

export interface ChannelsSlice {
  channels: ChannelSummary[];
  channelsLoading: boolean;
  activeChannelId: number | null;
  activeRender: ChannelRender | null;
  activeProvenance: RenderProvenance[];
  activeChangelog: ChannelChangelog | null;
  renderLoading: boolean;
  renderError: string | null;
  loadChannels: () => Promise<void>;
  selectChannel: (id: number) => Promise<void>;
  renderChannel: (id: number) => Promise<void>;
  loadProvenance: (renderId: number) => Promise<void>;
  loadChangelog: (channelId: number) => Promise<void>;
  refreshChannelSources: (channelId: number) => Promise<void>;
  createChannel: (slug: string, title: string, description: string, topicQuery: string[]) => Promise<void>;
  deleteChannel: (channelId: number) => Promise<void>;
}

export const createChannelsSlice: StateCreator<AppStore, [], [], ChannelsSlice> = (set, get) => ({
  channels: [],
  channelsLoading: false,
  activeChannelId: null,
  activeRender: null,
  activeProvenance: [],
  activeChangelog: null,
  renderLoading: false,
  renderError: null,

  loadChannels: async () => {
    set({ channelsLoading: true });
    try {
      const channels = await invoke<ChannelSummary[]>('list_channels');
      set({ channels, channelsLoading: false });
    } catch {
      set({ channelsLoading: false });
    }
  },

  selectChannel: async (id: number) => {
    set({ activeChannelId: id, renderLoading: true, renderError: null, activeProvenance: [], activeChangelog: null });
    try {
      const render = await invoke<ChannelRender | null>('get_channel_content', { channelId: id });
      set({ activeRender: render, renderLoading: false });

      // Auto-load provenance and changelog
      if (render) {
        get().loadProvenance(render.id);
        get().loadChangelog(id);
      }
    } catch (error) {
      set({ renderLoading: false, renderError: `${error}` });
    }
  },

  renderChannel: async (id: number) => {
    set({ renderLoading: true, renderError: null });
    try {
      const render = await invoke<ChannelRender>('render_channel_now', { channelId: id });
      set({ activeRender: render, renderLoading: false });

      // Reload channels list to update freshness badges
      get().loadChannels();

      // Load provenance and changelog for new render
      if (render) {
        get().loadProvenance(render.id);
        get().loadChangelog(id);
      }
    } catch (error) {
      set({ renderLoading: false, renderError: `${error}` });
    }
  },

  loadProvenance: async (renderId: number) => {
    try {
      const provenance = await invoke<RenderProvenance[]>('get_channel_provenance', { renderId });
      set({ activeProvenance: provenance });
    } catch {
      // Silently ignore — provenance is supplementary
    }
  },

  loadChangelog: async (channelId: number) => {
    try {
      const changelog = await invoke<ChannelChangelog | null>('get_channel_changelog', { channelId });
      set({ activeChangelog: changelog });
    } catch {
      // Silently ignore — changelog is supplementary
    }
  },

  refreshChannelSources: async (channelId: number) => {
    try {
      await invoke<number>('refresh_channel_sources', { channelId });
      // Reload channels to update source counts
      get().loadChannels();
    } catch {
      // Silently ignore
    }
  },

  createChannel: async (slug, title, description, topicQuery) => {
    await invoke<number>('create_custom_channel', { slug, title, description, topicQuery });
    get().loadChannels();
  },

  deleteChannel: async (channelId) => {
    await invoke<void>('delete_channel', { channelId });
    const state = get();
    if (state.activeChannelId === channelId) {
      set({ activeChannelId: null, activeRender: null });
    }
    get().loadChannels();
  },
});
