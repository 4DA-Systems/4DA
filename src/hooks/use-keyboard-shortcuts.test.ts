import { describe, it, expect, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useKeyboardShortcuts } from './use-keyboard-shortcuts';

function makeActions(overrides = {}) {
  return {
    onAnalyze: vi.fn(),
    onToggleFilters: vi.fn(),
    onToggleBriefing: vi.fn(),
    onOpenSettings: vi.fn(),
    onEscape: vi.fn(),
    onHelp: vi.fn(),
    analyzeDisabled: false,
    briefingAvailable: true,
    filtersAvailable: true,
    ...overrides,
  };
}

function pressKey(key: string, options: Record<string, unknown> = {}) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, ...options }));
}

describe('useKeyboardShortcuts', () => {
  it('calls onAnalyze when "r" key is pressed', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('r');
    expect(actions.onAnalyze).toHaveBeenCalledTimes(1);
  });

  it('calls onToggleFilters when "f" key is pressed', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('f');
    expect(actions.onToggleFilters).toHaveBeenCalledTimes(1);
  });

  it('calls onToggleBriefing when "b" key is pressed', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('b');
    expect(actions.onToggleBriefing).toHaveBeenCalledTimes(1);
  });

  it('calls onOpenSettings when "," key is pressed', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey(',');
    expect(actions.onOpenSettings).toHaveBeenCalledTimes(1);
  });

  it('calls onEscape when Escape key is pressed', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('Escape');
    expect(actions.onEscape).toHaveBeenCalledTimes(1);
  });

  it('calls onHelp when "?" key is pressed', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('?');
    expect(actions.onHelp).toHaveBeenCalledTimes(1);
  });

  it('does NOT fire onAnalyze when typing in an input element', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    // Create an input element and simulate typing in it
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();

    const event = new KeyboardEvent('keydown', { key: 'r', bubbles: true });
    Object.defineProperty(event, 'target', { value: input });
    window.dispatchEvent(event);

    expect(actions.onAnalyze).not.toHaveBeenCalled();
    document.body.removeChild(input);
  });

  it('does NOT fire when typing in a textarea element', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    const textarea = document.createElement('textarea');
    document.body.appendChild(textarea);
    textarea.focus();

    const event = new KeyboardEvent('keydown', { key: 'f', bubbles: true });
    Object.defineProperty(event, 'target', { value: textarea });
    window.dispatchEvent(event);

    expect(actions.onToggleFilters).not.toHaveBeenCalled();
    document.body.removeChild(textarea);
  });

  it('does NOT fire when typing in a select element', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    const select = document.createElement('select');
    document.body.appendChild(select);
    select.focus();

    const event = new KeyboardEvent('keydown', { key: 'b', bubbles: true });
    Object.defineProperty(event, 'target', { value: select });
    window.dispatchEvent(event);

    expect(actions.onToggleBriefing).not.toHaveBeenCalled();
    document.body.removeChild(select);
  });

  it('does NOT fire onAnalyze when Ctrl key is held', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('r', { ctrlKey: true });
    expect(actions.onAnalyze).not.toHaveBeenCalled();
  });

  it('does NOT fire onAnalyze when Meta key is held', () => {
    const actions = makeActions();
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('r', { metaKey: true });
    expect(actions.onAnalyze).not.toHaveBeenCalled();
  });

  it('does NOT fire onAnalyze when analyzeDisabled is true', () => {
    const actions = makeActions({ analyzeDisabled: true });
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('r');
    expect(actions.onAnalyze).not.toHaveBeenCalled();
  });

  it('does NOT fire onToggleBriefing when briefingAvailable is false', () => {
    const actions = makeActions({ briefingAvailable: false });
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('b');
    expect(actions.onToggleBriefing).not.toHaveBeenCalled();
  });

  it('does NOT fire onToggleFilters when filtersAvailable is false', () => {
    const actions = makeActions({ filtersAvailable: false });
    renderHook(() => useKeyboardShortcuts(actions));

    pressKey('f');
    expect(actions.onToggleFilters).not.toHaveBeenCalled();
  });
});
