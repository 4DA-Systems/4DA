import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore, FeedbackSlice, TopicAffinity, AntiTopic } from './types';

export const createFeedbackSlice: StateCreator<AppStore, [], [], FeedbackSlice> = (set) => ({
  feedbackGiven: {},
  learnedAffinities: [],
  antiTopics: [],
  lastLearnedTopic: null,

  setLastLearnedTopic: (topic) => set({ lastLearnedTopic: topic }),

  setFeedbackGivenFull: (updater) => {
    set(state => ({
      feedbackGiven: typeof updater === 'function' ? updater(state.feedbackGiven) : updater,
    }));
  },

  loadLearnedBehavior: async () => {
    try {
      const affinityResult = await invoke<{
        affinities: TopicAffinity[];
        count: number;
      }>('ace_get_topic_affinities');

      if (affinityResult.affinities) {
        const sorted = [...affinityResult.affinities].sort(
          (a, b) => Math.abs(b.affinity_score) - Math.abs(a.affinity_score),
        );
        set({ learnedAffinities: sorted });
      }

      const antiResult = await invoke<{
        anti_topics: AntiTopic[];
        count: number;
      }>('ace_get_anti_topics', { min_rejections: 2 });

      if (antiResult.anti_topics) {
        set({ antiTopics: antiResult.anti_topics });
      }
    } catch (error) {
      console.debug('Learned behavior not available:', error);
    }
  },
});
