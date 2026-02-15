import { useEffect } from 'react';
import { useAppStore } from '../store';

/**
 * Feedback hook — thin wrapper around Zustand store.
 * All state and actions (including recordInteraction) live in the store.
 * This hook triggers loadLearnedBehavior on mount.
 */
export function useFeedback() {
  const feedbackGiven = useAppStore(s => s.feedbackGiven);
  const learnedAffinities = useAppStore(s => s.learnedAffinities);
  const antiTopics = useAppStore(s => s.antiTopics);
  const lastLearnedTopic = useAppStore(s => s.lastLearnedTopic);
  const loadLearnedBehavior = useAppStore(s => s.loadLearnedBehavior);
  const loadPersistedSavedIds = useAppStore(s => s.loadPersistedSavedIds);
  const recordInteraction = useAppStore(s => s.recordInteraction);

  useEffect(() => {
    loadLearnedBehavior();
    loadPersistedSavedIds();
  }, [loadLearnedBehavior, loadPersistedSavedIds]);

  return {
    feedbackGiven,
    learnedAffinities,
    antiTopics,
    lastLearnedTopic,
    loadLearnedBehavior,
    recordInteraction,
  };
}
