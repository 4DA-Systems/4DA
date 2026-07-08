// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ViewErrorBoundary } from '../ViewErrorBoundary';

// Mock i18n the same way the component uses it
vi.mock('../../i18n', () => ({
  default: {
    t: (key: string, opts?: Record<string, string>) => {
      if (key === 'error.viewFailed' && opts?.viewName) {
        return `${opts.viewName} failed to load`;
      }
      if (key === 'error.viewRecovery') {
        return 'An unexpected error occurred. You can retry loading this view.';
      }
      if (key === 'error.retry') return 'Retry';
      return opts?.defaultValue ?? key;
    },
  },
}));

// Helper: a child that throws on render
function ThrowingChild({ shouldThrow = true }: { shouldThrow?: boolean }) {
  if (shouldThrow) throw new Error('Test render error');
  return <div>Child content</div>;
}

// Suppress React error boundary console noise during tests
beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
});

describe('ViewErrorBoundary', () => {
  it('renders children normally when no error', () => {
    render(
      <ViewErrorBoundary viewName="TestView">
        <div>Normal content</div>
      </ViewErrorBoundary>,
    );
    expect(screen.getByText('Normal content')).toBeInTheDocument();
  });

  it('catches error and shows role="alert"', () => {
    render(
      <ViewErrorBoundary viewName="Briefing">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('displays viewName in error message', () => {
    render(
      <ViewErrorBoundary viewName="Decisions">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    expect(screen.getByText('Decisions failed to load')).toBeInTheDocument();
  });

  it('Retry button resets error state and renders children again', () => {
    let shouldThrow = true;
    function ConditionalChild() {
      if (shouldThrow) throw new Error('Conditional error');
      return <div>Recovered content</div>;
    }

    render(
      <ViewErrorBoundary viewName="Profile">
        <ConditionalChild />
      </ViewErrorBoundary>,
    );

    // Error state is shown
    expect(screen.getByRole('alert')).toBeInTheDocument();

    // Fix the child before retrying
    shouldThrow = false;
    fireEvent.click(screen.getByText('Retry'));

    // Children render again
    expect(screen.getByText('Recovered content')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('calls onReset callback when Retry is clicked', () => {
    const onReset = vi.fn();
    let shouldThrow = true;
    function ConditionalChild() {
      if (shouldThrow) throw new Error('Reset error');
      return <div>OK</div>;
    }

    render(
      <ViewErrorBoundary viewName="Toolkit" onReset={onReset}>
        <ConditionalChild />
      </ViewErrorBoundary>,
    );

    shouldThrow = false;
    fireEvent.click(screen.getByText('Retry'));
    expect(onReset).toHaveBeenCalledTimes(1);
  });

  it('does NOT show stack trace', () => {
    render(
      <ViewErrorBoundary viewName="Coach">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    // The alert should not contain a <pre> element or the stack trace text
    const alert = screen.getByRole('alert');
    expect(alert.querySelector('pre')).toBeNull();
    expect(alert.textContent).not.toContain('at ThrowingChild');
  });

  it('does NOT show a Reload button', () => {
    render(
      <ViewErrorBoundary viewName="Saved">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    expect(screen.queryByText('Reload')).not.toBeInTheDocument();
    expect(screen.queryByText(/reload/i)).not.toBeInTheDocument();
  });

  // Regression: a crash in one view used to leave the single boundary instance
  // (reused across the tab-switch ternary) stuck in its error state, so the user
  // could not navigate to any other tab without a full app refresh.
  it('auto-clears the error when viewName changes (navigating to another tab)', () => {
    const { rerender } = render(
      <ViewErrorBoundary viewName="Signal">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    expect(screen.getByText('Signal failed to load')).toBeInTheDocument();

    // Simulate switching to a different tab: same boundary slot, new viewName,
    // healthy children. The boundary must recover on its own — no Retry click.
    rerender(
      <ViewErrorBoundary viewName="Briefing">
        <div>Briefing content</div>
      </ViewErrorBoundary>,
    );
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByText('Briefing content')).toBeInTheDocument();
  });

  // Regression: crashing the Graph view used to block the List view behind the
  // same boundary. Bumping resetKey (the List/Graph toggle) must recover.
  it('auto-clears the error when resetKey changes (toggling Signal List/Graph)', () => {
    const { rerender } = render(
      <ViewErrorBoundary viewName="Signal" resetKey="graph">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();

    rerender(
      <ViewErrorBoundary viewName="Signal" resetKey="list">
        <div>List content</div>
      </ViewErrorBoundary>,
    );
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByText('List content')).toBeInTheDocument();
  });

  it('keeps showing the error while viewName and resetKey are unchanged', () => {
    const { rerender } = render(
      <ViewErrorBoundary viewName="Signal" resetKey="graph">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();

    // A re-render with the SAME identity (still the crashed Graph view) must not
    // spuriously clear — otherwise it would loop: clear -> re-throw -> clear.
    rerender(
      <ViewErrorBoundary viewName="Signal" resetKey="graph">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('logs error via console.error in componentDidCatch', () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    render(
      <ViewErrorBoundary viewName="Channels">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );

    expect(consoleSpy).toHaveBeenCalled();
    const callArgs = consoleSpy.mock.calls.find(
      (args) => typeof args[0] === 'string' && args[0].includes('ViewErrorBoundary [Channels]'),
    );
    expect(callArgs).toBeDefined();
  });
});
