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
      <ViewErrorBoundary viewName="Insights">
        <ThrowingChild />
      </ViewErrorBoundary>,
    );
    expect(screen.getByText('Insights failed to load')).toBeInTheDocument();
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
