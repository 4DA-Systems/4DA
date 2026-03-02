import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('coach-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has streetsTier set to playbook', () => {
      expect(useAppStore.getState().streetsTier).toBe('playbook');
    });

    it('has empty coachSessions', () => {
      expect(useAppStore.getState().coachSessions).toEqual([]);
    });

    it('has activeSessionId null', () => {
      expect(useAppStore.getState().activeSessionId).toBeNull();
    });

    it('has empty coachMessages', () => {
      expect(useAppStore.getState().coachMessages).toEqual([]);
    });

    it('has coachLoading false', () => {
      expect(useAppStore.getState().coachLoading).toBe(false);
    });

    it('has engineRecommendation null', () => {
      expect(useAppStore.getState().engineRecommendation).toBeNull();
    });

    it('has strategyDocument null', () => {
      expect(useAppStore.getState().strategyDocument).toBeNull();
    });

    it('has launchReview null', () => {
      expect(useAppStore.getState().launchReview).toBeNull();
    });

    it('has empty coachNudges', () => {
      expect(useAppStore.getState().coachNudges).toEqual([]);
    });

    it('has empty templates', () => {
      expect(useAppStore.getState().templates).toEqual([]);
    });

    it('has empty videoCurriculum', () => {
      expect(useAppStore.getState().videoCurriculum).toEqual([]);
    });

    it('has videoStatus null', () => {
      expect(useAppStore.getState().videoStatus).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadStreetsTier
  // ---------------------------------------------------------------------------
  describe('loadStreetsTier', () => {
    it('sets streetsTier from invoke result', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({ tier: 'mentor', expired: false });

      await useAppStore.getState().loadStreetsTier();

      expect(invoke).toHaveBeenCalledWith('get_streets_tier');
      expect(useAppStore.getState().streetsTier).toBe('mentor');
    });

    it('falls back to playbook when expired', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({ tier: 'mentor', expired: true });

      await useAppStore.getState().loadStreetsTier();

      expect(useAppStore.getState().streetsTier).toBe('playbook');
    });

    it('falls back to playbook on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadStreetsTier();

      expect(useAppStore.getState().streetsTier).toBe('playbook');
    });
  });

  // ---------------------------------------------------------------------------
  // activateStreetsLicense
  // ---------------------------------------------------------------------------
  describe('activateStreetsLicense', () => {
    it('returns true and updates state on success', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({
        success: true,
        streets_tier: 'mentor',
        tier: 'pro',
      });

      const result = await useAppStore.getState().activateStreetsLicense('KEY-123');

      expect(invoke).toHaveBeenCalledWith('activate_streets_license', { licenseKey: 'KEY-123' });
      expect(result).toBe(true);
      expect(useAppStore.getState().streetsTier).toBe('mentor');
      expect(useAppStore.getState().tier).toBe('pro');
    });

    it('returns false when result.success is false', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({ success: false, streets_tier: '', tier: '' });

      const result = await useAppStore.getState().activateStreetsLicense('BAD-KEY');

      expect(result).toBe(false);
    });

    it('returns false on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      const result = await useAppStore.getState().activateStreetsLicense('KEY');

      expect(result).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadCoachSessions
  // ---------------------------------------------------------------------------
  describe('loadCoachSessions', () => {
    it('loads sessions from backend', async () => {
      const mockSessions = [
        { id: 's1', type: 'general', title: 'Session 1', created_at: '2024-01-01', updated_at: '2024-01-01' },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockSessions);

      await useAppStore.getState().loadCoachSessions();

      expect(invoke).toHaveBeenCalledWith('coach_list_sessions');
      expect(useAppStore.getState().coachSessions).toEqual(mockSessions);
    });

    it('silently handles errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadCoachSessions();

      expect(useAppStore.getState().coachSessions).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // createCoachSession
  // ---------------------------------------------------------------------------
  describe('createCoachSession', () => {
    it('creates session and updates state', async () => {
      const mockSession = { id: 'new-1', type: 'strategy', title: 'My Session', created_at: '2024-01-01', updated_at: '2024-01-01' };
      vi.mocked(invoke).mockResolvedValueOnce(mockSession);

      const id = await useAppStore.getState().createCoachSession('strategy', 'My Session');

      expect(invoke).toHaveBeenCalledWith('coach_create_session', { sessionType: 'strategy', title: 'My Session' });
      expect(id).toBe('new-1');
      expect(useAppStore.getState().activeSessionId).toBe('new-1');
      expect(useAppStore.getState().coachSessions).toHaveLength(1);
      expect(useAppStore.getState().coachMessages).toEqual([]);
    });

    it('returns null on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      const id = await useAppStore.getState().createCoachSession('chat');

      expect(id).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // deleteCoachSession
  // ---------------------------------------------------------------------------
  describe('deleteCoachSession', () => {
    it('removes session from list', async () => {
      // Set up existing sessions
      useAppStore.setState({
        coachSessions: [
          { id: 's1', type: 'general', title: 'A', created_at: '', updated_at: '' },
          { id: 's2', type: 'general', title: 'B', created_at: '', updated_at: '' },
        ] as never[],
        activeSessionId: 's1',
      });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useAppStore.getState().deleteCoachSession('s1');

      expect(invoke).toHaveBeenCalledWith('coach_delete_session', { sessionId: 's1' });
      expect(useAppStore.getState().coachSessions).toHaveLength(1);
      expect(useAppStore.getState().activeSessionId).toBeNull();
    });

    it('silently handles errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().deleteCoachSession('nonexistent');

      // No throw
    });
  });

  // ---------------------------------------------------------------------------
  // setActiveSession
  // ---------------------------------------------------------------------------
  describe('setActiveSession', () => {
    it('sets activeSessionId and clears messages', () => {
      // Mock loadCoachHistory which gets called for non-null ids
      vi.mocked(invoke).mockResolvedValueOnce([]);

      useAppStore.setState({ coachMessages: [{ id: 1 }] as never[] });
      useAppStore.getState().setActiveSession('s1');

      expect(useAppStore.getState().activeSessionId).toBe('s1');
      expect(useAppStore.getState().coachMessages).toEqual([]);
    });

    it('sets to null without loading history', () => {
      useAppStore.getState().setActiveSession(null);

      expect(useAppStore.getState().activeSessionId).toBeNull();
      expect(invoke).not.toHaveBeenCalledWith('coach_get_history', expect.anything());
    });
  });

  // ---------------------------------------------------------------------------
  // sendCoachMessage
  // ---------------------------------------------------------------------------
  describe('sendCoachMessage', () => {
    it('adds user message optimistically then appends response', async () => {
      useAppStore.setState({ activeSessionId: 'sess-1' });
      const mockResponse = {
        id: 100,
        session_id: 'sess-1',
        role: 'assistant',
        content: 'Hello!',
        token_count: 10,
        cost_cents: 1,
        created_at: '2024-01-01',
      };
      vi.mocked(invoke).mockResolvedValueOnce(mockResponse);

      await useAppStore.getState().sendCoachMessage('Hi');

      expect(invoke).toHaveBeenCalledWith('coach_send_message', { sessionId: 'sess-1', content: 'Hi' });
      const messages = useAppStore.getState().coachMessages;
      expect(messages).toHaveLength(2); // user + assistant
      expect(messages[0].role).toBe('user');
      expect(messages[1].role).toBe('assistant');
      expect(useAppStore.getState().coachLoading).toBe(false);
    });

    it('does nothing without active session', async () => {
      useAppStore.setState({ activeSessionId: null });

      await useAppStore.getState().sendCoachMessage('Hi');

      expect(invoke).not.toHaveBeenCalled();
    });

    it('resets coachLoading on error', async () => {
      useAppStore.setState({ activeSessionId: 'sess-1' });
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().sendCoachMessage('Hi');

      expect(useAppStore.getState().coachLoading).toBe(false);
    });
  });

});

