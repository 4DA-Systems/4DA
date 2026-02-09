import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { HNRelevance, FeedbackAction, FeedbackGiven } from '../types';

export function useFeedback(onStatusChange?: (status: string) => void) {
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
      console.log('Learned behavior not available:', error);
    }
  }, []);

  const recordInteraction = useCallback(async (
    itemId: number,
    actionType: FeedbackAction,
    item: HNRelevance,
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

      if (onStatusChange) {
        // Show what was learned with specific topics
        const topTopics = topics.slice(0, 3).join(', ');
        const learnMessage = actionType === 'save'
          ? `✓ Saved • Learning: +${topTopics || 'relevance'}`
          : actionType === 'mark_irrelevant'
          ? `⊘ Irrelevant • Learning: -${topTopics || 'this type'}`
          : actionType === 'dismiss'
          ? '✗ Dismissed • Noted for future filtering'
          : 'Opened • Tracking engagement';

        onStatusChange(learnMessage);
        setTimeout(() => onStatusChange('Ready'), 3000);
      }

      setTimeout(loadLearnedBehavior, 500);
    } catch (error) {
      console.error('Failed to record interaction:', error);
    }
  }, [loadLearnedBehavior, onStatusChange]);

  useEffect(() => {
    loadLearnedBehavior();
  }, [loadLearnedBehavior]);

  return {
    feedbackGiven,
    learnedAffinities,
    antiTopics,
    loadLearnedBehavior,
    recordInteraction,
  };
}
