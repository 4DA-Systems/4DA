import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ToastContainer } from './Toast';
import type { Toast } from '../store/types';

function makeToast(overrides: Partial<Toast> = {}): Toast {
  return {
    id: 1,
    type: 'info',
    message: 'Test toast message',
    ...overrides,
  };
}

describe('ToastContainer', () => {
  it('returns null when toasts array is empty', () => {
    const { container } = render(<ToastContainer toasts={[]} onDismiss={vi.fn()} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders a single toast with its message', () => {
    const toast = makeToast({ message: 'Operation succeeded' });
    render(<ToastContainer toasts={[toast]} onDismiss={vi.fn()} />);
    expect(screen.getByText('Operation succeeded')).toBeInTheDocument();
  });

  it('renders multiple toasts', () => {
    const toasts = [
      makeToast({ id: 1, message: 'First toast' }),
      makeToast({ id: 2, message: 'Second toast', type: 'error' }),
    ];
    render(<ToastContainer toasts={toasts} onDismiss={vi.fn()} />);
    expect(screen.getByText('First toast')).toBeInTheDocument();
    expect(screen.getByText('Second toast')).toBeInTheDocument();
  });

  it('calls onDismiss with toast id when dismiss button is clicked', () => {
    const onDismiss = vi.fn();
    const toast = makeToast({ id: 42, message: 'Dismissable toast' });
    render(<ToastContainer toasts={[toast]} onDismiss={onDismiss} />);

    const dismissBtn = screen.getByLabelText('action.dismiss');
    fireEvent.click(dismissBtn);
    expect(onDismiss).toHaveBeenCalledWith(42);
  });

  it('renders error toast with role="alert"', () => {
    const toast = makeToast({ type: 'error', message: 'Error occurred' });
    render(<ToastContainer toasts={[toast]} onDismiss={vi.fn()} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('renders non-error toasts with role="status"', () => {
    const toast = makeToast({ type: 'success', message: 'All good' });
    render(<ToastContainer toasts={[toast]} onDismiss={vi.fn()} />);
    // The container div has role="status" and each success toast also has role="status"
    const statusElements = screen.getAllByRole('status');
    expect(statusElements.length).toBeGreaterThanOrEqual(1);
  });

  it('renders action button when toast has an action', () => {
    const actionFn = vi.fn();
    const toast = makeToast({
      message: 'Action toast',
      action: { label: 'Undo', onClick: actionFn },
    });
    const onDismiss = vi.fn();
    render(<ToastContainer toasts={[toast]} onDismiss={onDismiss} />);

    const actionBtn = screen.getByLabelText('Undo');
    expect(actionBtn).toBeInTheDocument();
    fireEvent.click(actionBtn);
    expect(actionFn).toHaveBeenCalledTimes(1);
    // Action button also dismisses the toast
    expect(onDismiss).toHaveBeenCalledWith(toast.id);
  });

  it('does not render action button when toast has no action', () => {
    const toast = makeToast({ message: 'No action toast' });
    render(<ToastContainer toasts={[toast]} onDismiss={vi.fn()} />);
    // Only the dismiss button should exist
    const buttons = screen.getAllByRole('button');
    expect(buttons).toHaveLength(1); // just the dismiss 'x' button
  });
});
