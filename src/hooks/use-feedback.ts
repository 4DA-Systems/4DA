// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect } from 'react';
import { useAppStore } from '../store';
import { runWhenIdle } from '../lib/defer';

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

  // Deferred to idle: learned affinities + persisted saved-item ids influence
  // ranking/badging but are not first-paint-critical, so they stay off the mount
  // IPC stampede (see src/lib/defer.ts).
  useEffect(() => {
    return runWhenIdle(() => {
      void loadLearnedBehavior();
      void loadPersistedSavedIds();
    });
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
