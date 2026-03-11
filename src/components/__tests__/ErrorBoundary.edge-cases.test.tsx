/**
 * ErrorBoundary edge case tests.
 *
 * Covers multiple children throwing, nested error boundaries,
 * different error types, and recovery after multiple errors.
 */
import { describe, it, expect, vi, beforeAll, afterAll } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ErrorBoundary } from '../ErrorBoundary';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('../../i18n', () => ({
  default: { t: (key: string) => key },
}));

// A component that throws
function ThrowingChild({ error }: { error: Error | string }): never {
  throw typeof error === 'string' ? new Error(error) : error;
}

// A component that may throw
function _ConditionalThrow({ shouldThrow, message }: { shouldThrow: boolean; message?: string }) {
  if (shouldThrow) throw new Error(message || 'Conditional error');
  return <div>Safe content</div>;
}

describe('ErrorBoundary edge cases', () => {
  const originalConsoleError = console.error;
  beforeAll(() => { console.error = vi.fn(); });
  afterAll(() => { console.error = originalConsoleError; });

  it('catches TypeError thrown by child', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild error={new TypeError('Cannot read property of null')} />
      </ErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText(/Cannot read property of null/)).toBeInTheDocument();
  });

  it('catches RangeError thrown by child', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild error={new RangeError('Maximum call stack size exceeded')} />
      </ErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText(/Maximum call stack size exceeded/)).toBeInTheDocument();
  });

  it('catches string error message', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild error="plain string error" />
      </ErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText(/plain string error/)).toBeInTheDocument();
  });

  it('shows error UI when one of multiple children throws', () => {
    render(
      <ErrorBoundary>
        <div>Safe child</div>
        <ThrowingChild error="One child fails" />
      </ErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
    // The safe child should not render since the boundary catches the whole subtree
    expect(screen.queryByText('Safe child')).not.toBeInTheDocument();
  });

  it('nested ErrorBoundary catches inner error without affecting outer', () => {
    render(
      <ErrorBoundary>
        <div>Outer content</div>
        <ErrorBoundary>
          <ThrowingChild error="Inner error only" />
        </ErrorBoundary>
      </ErrorBoundary>,
    );
    // Outer content should still render (inner boundary catches)
    expect(screen.getByText('Outer content')).toBeInTheDocument();
    // One alert from the inner boundary
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('shows Try Recover and Reload buttons in error state', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild error="test error" />
      </ErrorBoundary>,
    );
    expect(screen.getByText('error.tryRecover')).toBeInTheDocument();
    expect(screen.getByText('error.reload')).toBeInTheDocument();
  });

  it('recovers after clicking Try Recover when child stops throwing', () => {
    let shouldThrow = true;
    function MaybeThrow() {
      if (shouldThrow) throw new Error('Temporary error');
      return <div>Recovered content</div>;
    }

    const { rerender } = render(
      <ErrorBoundary>
        <MaybeThrow />
      </ErrorBoundary>,
    );

    expect(screen.getByRole('alert')).toBeInTheDocument();

    shouldThrow = false;
    fireEvent.click(screen.getByText('error.tryRecover'));
    rerender(
      <ErrorBoundary>
        <MaybeThrow />
      </ErrorBoundary>,
    );

    expect(screen.getByText('Recovered content')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('displays the translated heading key in fallback UI', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild error="heading test" />
      </ErrorBoundary>,
    );
    expect(screen.getByText('error.somethingWrong')).toBeInTheDocument();
  });

  it('error state persists if child keeps throwing after recovery attempt', () => {
    function AlwaysThrow() {
      throw new Error('Persistent error');
    }

    render(
      <ErrorBoundary>
        <AlwaysThrow />
      </ErrorBoundary>,
    );

    expect(screen.getByRole('alert')).toBeInTheDocument();
    fireEvent.click(screen.getByText('error.tryRecover'));
    // Should still be in error state since child keeps throwing
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });
});
