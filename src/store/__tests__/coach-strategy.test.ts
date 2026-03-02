import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('coach-slice strategy/nudges/templates', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // recommendEngines
  // ---------------------------------------------------------------------------
  describe('recommendEngines', () => {
    it('sets engineRecommendation on success', async () => {
      const mockRec = { engines: [{ name: 'ollama', score: 0.9 }] };
      vi.mocked(invoke).mockResolvedValueOnce(mockRec);

      await useAppStore.getState().recommendEngines();

      expect(invoke).toHaveBeenCalledWith('coach_recommend_engines');
      expect(useAppStore.getState().engineRecommendation).toEqual(mockRec);
      expect(useAppStore.getState().coachLoading).toBe(false);
    });

    it('resets loading on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().recommendEngines();

      expect(useAppStore.getState().coachLoading).toBe(false);
      expect(useAppStore.getState().engineRecommendation).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // generateStrategy
  // ---------------------------------------------------------------------------
  describe('generateStrategy', () => {
    it('sets strategyDocument on success', async () => {
      vi.mocked(invoke).mockResolvedValueOnce('# My Strategy');

      await useAppStore.getState().generateStrategy();

      expect(invoke).toHaveBeenCalledWith('coach_generate_strategy');
      expect(useAppStore.getState().strategyDocument).toBe('# My Strategy');
      expect(useAppStore.getState().coachLoading).toBe(false);
    });

    it('resets loading on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().generateStrategy();

      expect(useAppStore.getState().coachLoading).toBe(false);
      expect(useAppStore.getState().strategyDocument).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // submitLaunchReview
  // ---------------------------------------------------------------------------
  describe('submitLaunchReview', () => {
    it('sets launchReview on success', async () => {
      const mockReview = { score: 85, feedback: 'Looks good' };
      vi.mocked(invoke).mockResolvedValueOnce(mockReview);

      await useAppStore.getState().submitLaunchReview('My awesome project');

      expect(invoke).toHaveBeenCalledWith('coach_launch_review', { projectDescription: 'My awesome project' });
      expect(useAppStore.getState().launchReview).toEqual(mockReview);
      expect(useAppStore.getState().coachLoading).toBe(false);
    });

    it('resets loading on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().submitLaunchReview('desc');

      expect(useAppStore.getState().coachLoading).toBe(false);
      expect(useAppStore.getState().launchReview).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // progressCheckIn
  // ---------------------------------------------------------------------------
  describe('progressCheckIn', () => {
    it('appends assistant message on success', async () => {
      vi.mocked(invoke).mockResolvedValueOnce('You are making great progress!');

      await useAppStore.getState().progressCheckIn();

      const messages = useAppStore.getState().coachMessages;
      expect(messages).toHaveLength(1);
      expect(messages[0].role).toBe('assistant');
      expect(messages[0].content).toBe('You are making great progress!');
      expect(useAppStore.getState().coachLoading).toBe(false);
    });

    it('resets loading on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().progressCheckIn();

      expect(useAppStore.getState().coachLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadCoachNudges / dismissNudge
  // ---------------------------------------------------------------------------
  describe('loadCoachNudges', () => {
    it('loads nudges from backend', async () => {
      const mockNudges = [{ id: 1, type: 'tip', message: 'Try this', priority: 1 }];
      vi.mocked(invoke).mockResolvedValueOnce(mockNudges);

      await useAppStore.getState().loadCoachNudges();

      expect(invoke).toHaveBeenCalledWith('get_coach_nudges');
      expect(useAppStore.getState().coachNudges).toEqual(mockNudges);
    });

    it('silently handles errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadCoachNudges();

      expect(useAppStore.getState().coachNudges).toEqual([]);
    });
  });

  describe('dismissNudge', () => {
    it('removes nudge from list', async () => {
      useAppStore.setState({
        coachNudges: [
          { id: 1, type: 'tip', message: 'A' },
          { id: 2, type: 'tip', message: 'B' },
        ] as never[],
      });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useAppStore.getState().dismissNudge(1);

      expect(invoke).toHaveBeenCalledWith('dismiss_coach_nudge', { nudgeId: 1 });
      expect(useAppStore.getState().coachNudges).toHaveLength(1);
      expect(useAppStore.getState().coachNudges[0]).toHaveProperty('id', 2);
    });
  });

  // ---------------------------------------------------------------------------
  // loadTemplates
  // ---------------------------------------------------------------------------
  describe('loadTemplates', () => {
    it('loads templates from backend', async () => {
      const mockTemplates = [{ id: 't1', name: 'Launch Plan', content: '...' }];
      vi.mocked(invoke).mockResolvedValueOnce(mockTemplates);

      await useAppStore.getState().loadTemplates();

      expect(invoke).toHaveBeenCalledWith('get_templates');
      expect(useAppStore.getState().templates).toEqual(mockTemplates);
    });

    it('silently handles errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadTemplates();

      expect(useAppStore.getState().templates).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // loadVideoCurriculum
  // ---------------------------------------------------------------------------
  describe('loadVideoCurriculum', () => {
    it('loads video lessons and status', async () => {
      const mockLessons = [{ id: 'v1', title: 'Lesson 1', duration: 300 }];
      const mockStatus = { total: 10, completed: 3, percent: 30 };
      vi.mocked(invoke).mockResolvedValueOnce([mockLessons, mockStatus]);

      await useAppStore.getState().loadVideoCurriculum();

      expect(invoke).toHaveBeenCalledWith('get_video_curriculum');
      expect(useAppStore.getState().videoCurriculum).toEqual(mockLessons);
      expect(useAppStore.getState().videoStatus).toEqual(mockStatus);
    });

    it('silently handles errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadVideoCurriculum();

      expect(useAppStore.getState().videoCurriculum).toEqual([]);
      expect(useAppStore.getState().videoStatus).toBeNull();
    });
  });
});
