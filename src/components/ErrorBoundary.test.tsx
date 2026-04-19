// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ErrorBoundary } from './ErrorBoundary';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

// Mock the i18n module used directly by ErrorBoundary (not through react-i18next hook)
vi.mock('../i18n', () => ({
  default: {
    t: (key: string) => key,
  },
}));

// A component that throws on render
function ThrowingChild({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) {
    throw new Error('Test error message');
  }
  return <div>Child rendered successfully</div>;
}

describe('ErrorBoundary', () => {
  // Suppress console.error for these tests since we expect errors
  const originalConsoleError = console.error;
  beforeAll(() => {
    console.error = vi.fn();
  });
  afterAll(() => {
    console.error = originalConsoleError;
  });

  it('renders children normally when no error occurs', () => {
    render(
      <ErrorBoundary>
        <div>Hello World</div>
      </ErrorBoundary>,
    );
    expect(screen.getByText('Hello World')).toBeInTheDocument();
  });

  it('renders multiple children without error', () => {
    render(
      <ErrorBoundary>
        <div>First child</div>
        <div>Second child</div>
      </ErrorBoundary>,
    );
    expect(screen.getByText('First child')).toBeInTheDocument();
    expect(screen.getByText('Second child')).toBeInTheDocument();
  });

  it('catches child errors and shows fallback UI with alert role', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.queryByText('Child rendered successfully')).not.toBeInTheDocument();
  });

  it('displays the error message in the fallback UI', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByText(/Test error message/)).toBeInTheDocument();
  });

  it('displays the i18n translated heading in the fallback UI', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild shouldThrow={true} />
      </ErrorBoundary>,
    );
    // The mock t() returns the key itself
    expect(screen.getByText('error.somethingWrong')).toBeInTheDocument();
  });

  it('shows a "Try Recover" button in the fallback UI', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByText('error.tryRecover')).toBeInTheDocument();
  });

  it('shows a "Reload" button in the fallback UI', () => {
    render(
      <ErrorBoundary>
        <ThrowingChild shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByText('error.reload')).toBeInTheDocument();
  });

  it('recovers and renders children again when "Try Recover" is clicked', () => {
    // Use a controlled component that can stop throwing
    let shouldThrow = true;
    function MaybeThrow() {
      if (shouldThrow) throw new Error('Controlled error');
      return <div>Recovered successfully</div>;
    }

    const { rerender } = render(
      <ErrorBoundary>
        <MaybeThrow />
      </ErrorBoundary>,
    );

    // Should be in error state
    expect(screen.getByRole('alert')).toBeInTheDocument();

    // Stop throwing and click recover
    shouldThrow = false;
    fireEvent.click(screen.getByText('error.tryRecover'));

    // After setState reset, it should attempt to re-render children
    // Need to rerender to see the updated component
    rerender(
      <ErrorBoundary>
        <MaybeThrow />
      </ErrorBoundary>,
    );

    expect(screen.getByText('Recovered successfully')).toBeInTheDocument();
  });
});
