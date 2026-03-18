import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { FeedbackAction } from '../types';
import type { AppStore, FeedbackSlice, AntiTopic } from './types';

// Client-side score adjustment multipliers for immediate feedback
const FEEDBACK_ADJUSTMENTS: Record<FeedbackAction, number> = {
  save: 0.10,
  click: 0.05,
  dismiss: -0.10,
  mark_irrelevant: -0.20,
};

const STOP_WORDS = new Set([
  'the', 'and', 'for', 'with', 'that', 'this', 'from', 'have', 'been',
  'will', 'what', 'when', 'where', 'which', 'about', 'into', 'your',
  'more', 'some',
]);

export const createFeedbackSlice: StateCreator<AppStore, [], [], FeedbackSlice> = (set, get) => ({
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

  loadPersistedSavedIds: async () => {
    try {
      const items = await cmd('get_saved_items');
      if (items.length > 0) {
        set(state => {
          const next = { ...state.feedbackGiven };
          for (const item of items) {
            if (!next[item.item_id]) {
              next[item.item_id] = 'save';
            }
          }
          return { feedbackGiven: next };
        });
      }
    } catch {
      /* persisted saved ids not available */
    }
  },

  loadLearnedBehavior: async () => {
    const [affinityResult, antiResult] = await Promise.allSettled([
      cmd('ace_get_topic_affinities'),
      cmd('ace_get_anti_topics', { min_rejections: 2 }),
    ]);

    if (affinityResult.status === 'fulfilled' && affinityResult.value.affinities) {
      const sorted = [...affinityResult.value.affinities].sort(
        (a, b) => Math.abs(b.affinity_score) - Math.abs(a.affinity_score),
      );
      set({ learnedAffinities: sorted });
    }

    if (antiResult.status === 'fulfilled' && antiResult.value.anti_topics) {
      set({ antiTopics: antiResult.value.anti_topics as unknown as AntiTopic[] });
    }
  },

  recordInteraction: async (itemId, actionType, item) => {
    try {
      const titleWords = item.title.toLowerCase().split(/\s+/);
      const topics = titleWords
        .filter(w => w.length > 3 && !STOP_WORDS.has(w))
        .slice(0, 5);

      const feedbackTypeMap: Record<string, string> = {
        save: 'save',
        dismiss: 'dismiss',
        mark_irrelevant: 'thumbs_down',
        click: 'click',
      };

      await Promise.all([
        cmd('ace_record_interaction', {
          item_id: itemId,
          action_type: actionType,
          action_data: null,
          item_topics: topics,
          item_source: item.source_type || 'hackernews',
        }),
        cmd('ace_record_accuracy_feedback', {
          item_id: itemId,
          predicted_score: item.top_score,
          feedback_type: feedbackTypeMap[actionType],
        }),
        // Feed the main DB feedback table — powers autophagy calibration analysis
        cmd('record_item_feedback', {
          item_id: itemId,
          relevant: actionType === 'save' || actionType === 'click',
        }),
      ]);

      // Update feedback state
      set(state => ({
        feedbackGiven: { ...state.feedbackGiven, [itemId]: actionType },
      }));

      // Track what was just learned for the visible learning loop
      const primaryTopic = topics[0] || null;
      if (primaryTopic) {
        const direction: 'positive' | 'negative' =
          (actionType === 'save' || actionType === 'click') ? 'positive' : 'negative';
        set({ lastLearnedTopic: { topic: primaryTopic, direction, timestamp: Date.now() } });
      }

      // Immediate score adjustment for visual feedback
      const delta = FEEDBACK_ADJUSTMENTS[actionType] ?? 0;
      if (delta !== 0) {
        get().setAppStateFull(s => ({
          ...s,
          relevanceResults: s.relevanceResults
            .map(r => r.id === itemId ? { ...r, top_score: Math.max(0, Math.min(1, r.top_score + delta)) } : r)
            .sort((a, b) => b.top_score - a.top_score),
        }));
      }

      // Fetch updated affinity for richer toast (non-critical)
      let affinityScore: number | null = null;
      if (primaryTopic) {
        try {
          const result = await cmd('ace_get_single_affinity', { topic: primaryTopic });
          if (result.affinity) {
            affinityScore = Math.round(result.affinity.affinity_score * 100);
          }
        } catch { /* non-critical */ }
      }

      // Show toast with undo action (except for click events)
      if (actionType !== 'click') {
        const { addToast } = get();
        const topicLabel = primaryTopic || 'this type';
        const scoreNote = affinityScore !== null ? ` (${affinityScore > 0 ? '+' : ''}${affinityScore}%)` : '';
        const learnMessage = actionType === 'save'
          ? `Saved — boosting '${topicLabel}'${scoreNote}. Similar content will rank higher next analysis.`
          : actionType === 'mark_irrelevant'
          ? `Got it — '${topicLabel}' added to anti-topics${scoreNote}. Matching content will be suppressed.`
          : `Noted — deprioritizing '${topicLabel}'${scoreNote}. 3 dismissals creates an auto-filter.`;

        addToast('success', learnMessage, {
          label: 'Undo',
          onClick: () => {
            // Revert feedback
            set(state => {
              const next = { ...state.feedbackGiven };
              delete next[itemId];
              return { feedbackGiven: next };
            });
            // Revert score adjustment
            if (delta !== 0) {
              get().setAppStateFull(s => ({
                ...s,
                relevanceResults: s.relevanceResults
                  .map(r => r.id === itemId ? { ...r, top_score: Math.max(0, Math.min(1, r.top_score - delta)) } : r)
                  .sort((a, b) => b.top_score - a.top_score),
              }));
            }
          },
        });
      }

      setTimeout(() => get().loadLearnedBehavior(), 500);
    } catch (error) {
      console.error('Failed to record interaction:', error);
    }
  },
});
