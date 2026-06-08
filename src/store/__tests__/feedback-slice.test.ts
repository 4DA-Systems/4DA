// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import { useAppStore } from '../index';
import { cmd } from '../../lib/commands';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// commands.ts exports only `cmd` at runtime (the rest are types), so a full mock
// is complete — no importOriginal/import() needed.
vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(() => Promise.resolve({})),
}));

const initialState = useAppStore.getState();
const cmdMock = cmd as unknown as Mock;

describe('feedback-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty feedbackGiven map', () => {
      expect(useAppStore.getState().feedbackGiven).toEqual({});
    });

    it('has empty learnedAffinities array', () => {
      expect(useAppStore.getState().learnedAffinities).toEqual([]);
    });

    it('has empty antiTopics array', () => {
      expect(useAppStore.getState().antiTopics).toEqual([]);
    });

    it('has lastLearnedTopic null', () => {
      expect(useAppStore.getState().lastLearnedTopic).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // setFeedbackGivenFull (direct value)
  // ---------------------------------------------------------------------------
  describe('setFeedbackGivenFull', () => {
    it('sets feedbackGiven when given a direct value', () => {
      const feedback = { 1: 'save' as const, 2: 'dismiss' as const };
      useAppStore.getState().setFeedbackGivenFull(feedback);
      expect(useAppStore.getState().feedbackGiven).toEqual(feedback);
    });

    it('replaces previous feedbackGiven entirely', () => {
      useAppStore.getState().setFeedbackGivenFull({ 1: 'save' });
      useAppStore.getState().setFeedbackGivenFull({ 5: 'click' });

      const feedback = useAppStore.getState().feedbackGiven;
      expect(feedback[5]).toBe('click');
      expect(feedback[1]).toBeUndefined();
    });

    it('accepts an updater function', () => {
      useAppStore.getState().setFeedbackGivenFull({ 1: 'save' });

      useAppStore.getState().setFeedbackGivenFull(prev => ({
        ...prev,
        2: 'dismiss',
      }));

      const feedback = useAppStore.getState().feedbackGiven;
      expect(feedback[1]).toBe('save');
      expect(feedback[2]).toBe('dismiss');
    });

    it('can remove entries via updater function', () => {
      useAppStore.getState().setFeedbackGivenFull({ 1: 'save', 2: 'click', 3: 'dismiss' });

      useAppStore.getState().setFeedbackGivenFull(prev => {
        const next = { ...prev };
        delete next[2];
        return next;
      });

      const feedback = useAppStore.getState().feedbackGiven;
      expect(feedback[1]).toBe('save');
      expect(feedback[2]).toBeUndefined();
      expect(feedback[3]).toBe('dismiss');
    });
  });

  // ---------------------------------------------------------------------------
  // Multiple feedback entries
  // ---------------------------------------------------------------------------
  describe('multiple feedback entries', () => {
    it('can accumulate feedback via sequential updater calls', () => {
      useAppStore.getState().setFeedbackGivenFull({ 10: 'save' });
      useAppStore.getState().setFeedbackGivenFull(prev => ({ ...prev, 20: 'click' }));
      useAppStore.getState().setFeedbackGivenFull(prev => ({ ...prev, 30: 'dismiss' }));
      useAppStore.getState().setFeedbackGivenFull(prev => ({ ...prev, 40: 'mark_irrelevant' }));

      const feedback = useAppStore.getState().feedbackGiven;
      expect(Object.keys(feedback)).toHaveLength(4);
      expect(feedback[10]).toBe('save');
      expect(feedback[20]).toBe('click');
      expect(feedback[30]).toBe('dismiss');
      expect(feedback[40]).toBe('mark_irrelevant');
    });

    it('can overwrite feedback for the same item id', () => {
      useAppStore.getState().setFeedbackGivenFull({ 1: 'click' });
      useAppStore.getState().setFeedbackGivenFull(prev => ({ ...prev, 1: 'save' }));

      expect(useAppStore.getState().feedbackGiven[1]).toBe('save');
    });

    it('can clear all feedback', () => {
      useAppStore.getState().setFeedbackGivenFull({ 1: 'save', 2: 'click', 3: 'dismiss' });
      useAppStore.getState().setFeedbackGivenFull({});

      expect(useAppStore.getState().feedbackGiven).toEqual({});
    });
  });

  // ---------------------------------------------------------------------------
  // setLastLearnedTopic
  // ---------------------------------------------------------------------------
  describe('setLastLearnedTopic', () => {
    it('sets a positive learned topic', () => {
      const topic = { topic: 'rust', direction: 'positive' as const, timestamp: Date.now() };
      useAppStore.getState().setLastLearnedTopic(topic);

      const result = useAppStore.getState().lastLearnedTopic;
      expect(result).not.toBeNull();
      expect(result!.topic).toBe('rust');
      expect(result!.direction).toBe('positive');
    });

    it('sets a negative learned topic', () => {
      const topic = { topic: 'spam', direction: 'negative' as const, timestamp: 1700000000000 };
      useAppStore.getState().setLastLearnedTopic(topic);

      const result = useAppStore.getState().lastLearnedTopic;
      expect(result!.topic).toBe('spam');
      expect(result!.direction).toBe('negative');
      expect(result!.timestamp).toBe(1700000000000);
    });

    it('can clear the learned topic by setting null', () => {
      useAppStore.getState().setLastLearnedTopic({ topic: 'test', direction: 'positive', timestamp: 0 });
      useAppStore.getState().setLastLearnedTopic(null);
      expect(useAppStore.getState().lastLearnedTopic).toBeNull();
    });

    it('replaces previous learned topic', () => {
      useAppStore.getState().setLastLearnedTopic({ topic: 'first', direction: 'positive', timestamp: 1 });
      useAppStore.getState().setLastLearnedTopic({ topic: 'second', direction: 'negative', timestamp: 2 });

      const result = useAppStore.getState().lastLearnedTopic;
      expect(result!.topic).toBe('second');
      expect(result!.direction).toBe('negative');
    });
  });

  // ---------------------------------------------------------------------------
  // recordInteraction — IPC contract guard (I-1 regression) + named failures
  // ---------------------------------------------------------------------------
  describe('recordInteraction IPC contract', () => {
    const sampleItem = {
      id: 41255,
      title: 'Axum 0.8 release notes',
      source_type: 'hackernews',
      top_score: 0.82,
    } as never;

    beforeEach(() => {
      cmdMock.mockReset();
      cmdMock.mockResolvedValue({});
    });

    it('invokes the engagement commands with camelCase args (I-1 regression guard)', async () => {
      await useAppStore.getState().recordInteraction(41255, 'save', sampleItem);

      const names = cmdMock.mock.calls.map(c => c[0]);
      expect(names).toContain('ace_record_interaction');
      expect(names).toContain('ace_record_accuracy_feedback');
      expect(names).toContain('record_item_feedback');

      const args = cmdMock.mock.calls.find(
        c => c[0] === 'ace_record_interaction',
      )?.[1] as Record<string, unknown>;
      // camelCase is the contract Tauri's deserializer expects (the I-1 fix).
      expect(args).toHaveProperty('itemId');
      expect(args).toHaveProperty('actionType');
      expect(args).toHaveProperty('itemTopics');
      expect(args).toHaveProperty('itemSource');
      // snake_case keys must NOT leak — that was the silent-failure root cause.
      expect(args).not.toHaveProperty('item_id');
      expect(args).not.toHaveProperty('action_type');
      expect(args).not.toHaveProperty('item_source');
    });

    it('surfaces the failing command name instead of swallowing it', async () => {
      cmdMock.mockImplementation((name: string) =>
        name === 'record_item_feedback'
          ? Promise.reject(new Error('contract mismatch'))
          : Promise.resolve({}),
      );

      await useAppStore.getState().recordInteraction(41255, 'save', sampleItem);

      const warning = useAppStore.getState().toasts.find(t => t.type === 'warning');
      expect(warning).toBeDefined();
      expect(warning!.message).toContain('record_item_feedback');
    });
  });
});
