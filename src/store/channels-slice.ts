// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore } from './types';
import type { ChannelSummary, ChannelRender, RenderProvenance, ChannelChangelog } from '../types/channels';

export interface ChannelsSlice {
  channels: ChannelSummary[];
  channelsLoading: boolean;
  channelsError: string | null;
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
  channelsError: null,
  activeChannelId: null,
  activeRender: null,
  activeProvenance: [],
  activeChangelog: null,
  renderLoading: false,
  renderError: null,

  loadChannels: async () => {
    set({ channelsLoading: true, channelsError: null });
    try {
      const channels = await cmd('list_channels') as unknown as ChannelSummary[];
      set({ channels, channelsLoading: false });
    } catch (e) {
      set({ channelsLoading: false, channelsError: String(e) });
    }
  },

  selectChannel: async (id: number) => {
    set({ activeChannelId: id, renderLoading: true, renderError: null, activeProvenance: [], activeChangelog: null });
    try {
      const render = await cmd('get_channel_content', { channelId: id });
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
      const render = await cmd('render_channel_now', { channelId: id });
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
      const provenance = await cmd('get_channel_provenance', { renderId });
      set({ activeProvenance: provenance });
    } catch {
      // Silently ignore — provenance is supplementary
    }
  },

  loadChangelog: async (channelId: number) => {
    try {
      const changelog = await cmd('get_channel_changelog', { channelId });
      set({ activeChangelog: changelog });
    } catch {
      // Silently ignore — changelog is supplementary
    }
  },

  refreshChannelSources: async (channelId: number) => {
    try {
      await cmd('refresh_channel_sources', { channelId });
      // Reload channels to update source counts
      get().loadChannels();
    } catch {
      // Silently ignore
    }
  },

  createChannel: async (slug, title, description, topicQuery) => {
    await cmd('create_custom_channel', { slug, title, description, topicQuery });
    get().loadChannels();
  },

  deleteChannel: async (channelId) => {
    await cmd('delete_channel', { channelId });
    const state = get();
    if (state.activeChannelId === channelId) {
      set({ activeChannelId: null, activeRender: null });
    }
    get().loadChannels();
  },
});
