// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import { extractTechTopics } from '../lib/known-tech';
import type { FeedbackAction } from '../types';
import type { AppStore, FeedbackSlice } from './types';

// Client-side score adjustment multipliers for immediate feedback
const FEEDBACK_ADJUSTMENTS: Record<FeedbackAction, number> = {
  save: 0.10,
  click: 0.05,
  dismiss: -0.10,
  mark_irrelevant: -0.20,
  snooze: -0.05,
};

export const createFeedbackSlice: StateCreator<AppStore, [], [], FeedbackSlice> = (set, get) => ({
  feedbackGiven: {},
  snoozedItemIds: new Set<number>(),

  loadSnoozedIds: async () => {
    try {
      const ids = await cmd('get_snoozed_item_ids');
      set({ snoozedItemIds: new Set(ids) });
    } catch {
      /* snoozed ids not available — items simply stay visible */
    }
  },

  markSnoozed: (itemId) => {
    set(state => {
      const next = new Set(state.snoozedItemIds);
      next.add(itemId);
      return { snoozedItemIds: next };
    });
  },

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

  recordInteraction: async (itemId, actionType, item) => {
    try {
      const topics = extractTechTopics(item.title);

      const feedbackTypeMap: Record<string, string> = {
        save: 'save',
        dismiss: 'dismiss',
        mark_irrelevant: 'thumbs_down',
        click: 'click',
      };

      // Optimistic UI update — card disappears immediately
      set(state => ({
        feedbackGiven: { ...state.feedbackGiven, [itemId]: actionType },
      }));

      const actionData = actionType === 'click'
        ? JSON.stringify({ type: 'click', dwell_time_seconds: 0, pattern: 'engaged' })
        : null;

      // Backend calls are non-blocking: one failure doesn't prevent the others.
      // Command names are tracked alongside the promises so a rejection is reported
      // WITH the command that failed. Silent IPC contract drift (the I-1 class bug —
      // camelCase/snake_case arg mismatches rejected and swallowed) must never again
      // disappear into an anonymous warning.
      const calls = [
        {
          name: 'ace_record_interaction',
          promise: cmd('ace_record_interaction', {
            itemId: itemId,
            actionType: actionType,
            actionData: actionData,
            itemTopics: topics,
            itemSource: item.source_type || 'hackernews',
          }),
        },
        {
          name: 'ace_record_accuracy_feedback',
          promise: cmd('ace_record_accuracy_feedback', {
            itemId: itemId,
            predictedScore: item.top_score,
            feedbackType: feedbackTypeMap[actionType]!,
          }),
        },
        {
          // Feed the main DB feedback table — powers autophagy calibration analysis
          name: 'record_item_feedback',
          promise: cmd('record_item_feedback', {
            itemId: itemId,
            relevant: actionType === 'save' || actionType === 'click',
          }),
        },
      ];
      const results = await Promise.allSettled(calls.map(c => c.promise));

      // Log any individual failures (named) without reverting the UI
      const failedCommands: string[] = [];
      results.forEach((r, i) => {
        if (r.status === 'rejected') {
          failedCommands.push(calls[i]!.name);
          console.warn(
            `Feedback command '${calls[i]!.name}' failed (non-blocking):`,
            r.reason,
          );
        }
      });

      // Notify user if any backend feedback calls failed, naming the command(s)
      if (failedCommands.length > 0) {
        get().addToast(
          'warning',
          `Feedback not fully saved (${failedCommands.join(', ')} failed)`,
        );
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

      // Show toast with undo action (except for click events).
      // Plain confirmations only — no learning promise. The implicit-capture
      // layer was removed in v20b (AD-031); feedback is recorded, not "learned from".
      if (actionType !== 'click') {
        const { addToast } = get();
        const confirmMessage = actionType === 'save'
          ? 'Saved.'
          : actionType === 'mark_irrelevant'
          ? 'Marked irrelevant.'
          : 'Dismissed.';

        addToast('success', confirmMessage, {
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
    } catch (error) {
      console.error('Failed to record interaction:', error);
    }
  },
});
