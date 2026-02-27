import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('playbook-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty playbookModules array', () => {
      expect(useAppStore.getState().playbookModules).toEqual([]);
    });

    it('has playbookContent null', () => {
      expect(useAppStore.getState().playbookContent).toBeNull();
    });

    it('has playbookProgress null', () => {
      expect(useAppStore.getState().playbookProgress).toBeNull();
    });

    it('has playbookLoading false', () => {
      expect(useAppStore.getState().playbookLoading).toBe(false);
    });

    it('has playbookError null', () => {
      expect(useAppStore.getState().playbookError).toBeNull();
    });

    it('has activeModuleId null', () => {
      expect(useAppStore.getState().activeModuleId).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // setActiveModuleId
  // ---------------------------------------------------------------------------
  describe('setActiveModuleId', () => {
    it('sets the active module id', () => {
      useAppStore.getState().setActiveModuleId('module-1');
      expect(useAppStore.getState().activeModuleId).toBe('module-1');
    });

    it('can clear the active module id', () => {
      useAppStore.getState().setActiveModuleId('module-1');
      useAppStore.getState().setActiveModuleId(null);
      expect(useAppStore.getState().activeModuleId).toBeNull();
    });

    it('replaces previous active module', () => {
      useAppStore.getState().setActiveModuleId('module-1');
      useAppStore.getState().setActiveModuleId('module-2');
      expect(useAppStore.getState().activeModuleId).toBe('module-2');
    });
  });

  // ---------------------------------------------------------------------------
  // loadPlaybookModules
  // ---------------------------------------------------------------------------
  describe('loadPlaybookModules', () => {
    it('sets playbookModules from invoke result', async () => {
      const mockModules = [
        { id: 'mod-1', title: 'Module 1', description: 'Desc', lesson_count: 3, icon: '📘' },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockModules);

      await useAppStore.getState().loadPlaybookModules();

      expect(invoke).toHaveBeenCalledWith('get_playbook_modules');
      expect(useAppStore.getState().playbookModules).toEqual(mockModules);
    });

    it('sets playbookError on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Network error'));

      await useAppStore.getState().loadPlaybookModules();

      expect(useAppStore.getState().playbookError).toBe('Error: Network error');
    });
  });

  // ---------------------------------------------------------------------------
  // loadPlaybookContent
  // ---------------------------------------------------------------------------
  describe('loadPlaybookContent', () => {
    it('sets loading state and content on success', async () => {
      const mockContent = { module_id: 'mod-1', title: 'Module 1', lessons: [] };
      vi.mocked(invoke).mockResolvedValueOnce(mockContent);

      await useAppStore.getState().loadPlaybookContent('mod-1');

      expect(useAppStore.getState().playbookLoading).toBe(false);
      expect(useAppStore.getState().playbookContent).toEqual(mockContent);
      expect(useAppStore.getState().activeModuleId).toBe('mod-1');
    });

    it('sets playbookError and stops loading on failure', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Not found'));

      await useAppStore.getState().loadPlaybookContent('mod-99');

      expect(useAppStore.getState().playbookLoading).toBe(false);
      expect(useAppStore.getState().playbookError).toBe('Error: Not found');
    });
  });
});
