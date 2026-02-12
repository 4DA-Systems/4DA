import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SourceRelevance, FeedbackAction, FeedbackGiven } from '../types';
import type { ToastAction } from './use-toasts';

// Client-side score adjustment multipliers for immediate feedback
const FEEDBACK_ADJUSTMENTS: Record<FeedbackAction, number> = {
  save: 0.10,           // Boost saved items
  click: 0.05,          // Small boost for engagement
  dismiss: -0.10,       // Sink dismissed items
  mark_irrelevant: -0.20, // Strong penalty for irrelevant
};

export function useFeedback(
  onStatusChange?: (status: string) => void,
  onScoreAdjust?: (itemId: number, delta: number) => void,
  addToast?: (type: 'success' | 'error' | 'warning' | 'info', message: string, action?: ToastAction) => void,
) {
  const [feedbackGiven, setFeedbackGiven] = useState<FeedbackGiven>({});
  const [learnedAffinities, setLearnedAffinities] = useState<Array<{
    topic: string;
    positive_signals: number;
    negative_signals: number;
    affinity_score: number;
  }>>([]);
  const [antiTopics, setAntiTopics] = useState<Array<{
    topic: string;
    rejection_count: number;
    confidence: number;
    auto_detected: boolean;
  }>>([]);
  const [lastLearnedTopic, setLastLearnedTopic] = useState<{
    topic: string;
    direction: 'positive' | 'negative';
    timestamp: number;
  } | null>(null);

  const loadLearnedBehavior = useCallback(async () => {
    try {
      const affinityResult = await invoke<{
        affinities: Array<{
          topic: string;
          positive_signals: number;
          negative_signals: number;
          affinity_score: number;
        }>;
        count: number;
      }>('ace_get_topic_affinities');

      if (affinityResult.affinities) {
        const sorted = [...affinityResult.affinities].sort(
          (a, b) => Math.abs(b.affinity_score) - Math.abs(a.affinity_score),
        );
        setLearnedAffinities(sorted);
      }

      const antiResult = await invoke<{
        anti_topics: Array<{
          topic: string;
          rejection_count: number;
          confidence: number;
          auto_detected: boolean;
        }>;
        count: number;
      }>('ace_get_anti_topics', { min_rejections: 2 });

      if (antiResult.anti_topics) {
        setAntiTopics(antiResult.anti_topics);
      }
    } catch (error) {
      console.debug('Learned behavior not available:', error);
    }
  }, []);

  const recordInteraction = useCallback(async (
    itemId: number,
    actionType: FeedbackAction,
    item: SourceRelevance,
  ) => {
    try {
      const titleWords = item.title.toLowerCase().split(/\s+/);
      const topics = titleWords.filter(w =>
        w.length > 3 &&
        !['the', 'and', 'for', 'with', 'that', 'this', 'from', 'have', 'been', 'will', 'what', 'when', 'where', 'which', 'about', 'into', 'your', 'more', 'some'].includes(w),
      ).slice(0, 5);

      await invoke('ace_record_interaction', {
        item_id: itemId,
        action_type: actionType,
        action_data: null,
        item_topics: topics,
        item_source: item.source_type || 'hackernews',
      });

      const feedbackTypeMap: Record<string, string> = {
        save: 'save',
        dismiss: 'dismiss',
        mark_irrelevant: 'thumbs_down',
        click: 'click',
      };

      await invoke('ace_record_accuracy_feedback', {
        item_id: itemId,
        predicted_score: item.top_score,
        feedback_type: feedbackTypeMap[actionType],
      });

      setFeedbackGiven(prev => ({ ...prev, [itemId]: actionType }));

      // Track what was just learned for the visible learning loop
      const primaryTopic = topics[0] || null;
      if (primaryTopic) {
        const direction: 'positive' | 'negative' =
          (actionType === 'save' || actionType === 'click') ? 'positive' : 'negative';
        setLastLearnedTopic({ topic: primaryTopic, direction, timestamp: Date.now() });
      }

      // Immediate score adjustment for visual feedback
      const delta = FEEDBACK_ADJUSTMENTS[actionType] ?? 0;
      if (delta !== 0 && onScoreAdjust) {
        onScoreAdjust(itemId, delta);
      }

      // Fetch updated affinity for richer toast (non-critical)
      let affinityScore: number | null = null;
      if (primaryTopic) {
        try {
          const result = await invoke<{ affinity: { topic: string; positive_signals: number; negative_signals: number; affinity_score: number } | null }>('ace_get_single_affinity', { topic: primaryTopic });
          if (result.affinity) {
            affinityScore = Math.round(result.affinity.affinity_score * 100);
          }
        } catch { /* non-critical */ }
      }

      // Show toast with undo action (except for click events)
      if (actionType !== 'click') {
        const topicLabel = primaryTopic || 'this type';
        const scoreNote = affinityScore !== null ? ` (${affinityScore > 0 ? '+' : ''}${affinityScore}%)` : '';
        const learnMessage = actionType === 'save'
          ? `Saved — boosting '${topicLabel}' in future results${scoreNote}`
          : actionType === 'mark_irrelevant'
          ? `Got it — filtering out '${topicLabel}'${scoreNote}`
          : `Noted — deprioritizing '${topicLabel}'${scoreNote}`;

        const undoAction: ToastAction = {
          label: 'Undo',
          onClick: () => {
            // Revert client-side feedback
            setFeedbackGiven(prev => {
              const next = { ...prev };
              delete next[itemId];
              return next;
            });
            // Revert score adjustment
            if (delta !== 0 && onScoreAdjust) {
              onScoreAdjust(itemId, -delta);
            }
          },
        };

        if (addToast) {
          addToast('success', learnMessage, undoAction);
        } else if (onStatusChange) {
          onStatusChange(learnMessage);
          setTimeout(() => onStatusChange('Ready'), 3000);
        }
      }

      setTimeout(loadLearnedBehavior, 500);
    } catch (error) {
      console.error('Failed to record interaction:', error);
    }
  }, [loadLearnedBehavior, onStatusChange, onScoreAdjust, addToast]);

  useEffect(() => {
    loadLearnedBehavior();
  }, [loadLearnedBehavior]);

  return {
    feedbackGiven,
    learnedAffinities,
    antiTopics,
    lastLearnedTopic,
    loadLearnedBehavior,
    recordInteraction,
  };
}
