import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import WaitlistSignup from '../WaitlistSignup';

describe('WaitlistSignup', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('renders team tier info', () => {
    render(<WaitlistSignup tier="team" />);
    expect(screen.getByText('4DA Team')).toBeInTheDocument();
    expect(screen.getByText('Coming Soon')).toBeInTheDocument();
    expect(screen.getByText('$29/seat/mo')).toBeInTheDocument();
  });

  it('renders enterprise tier info', () => {
    render(<WaitlistSignup tier="enterprise" />);
    expect(screen.getByText('4DA Enterprise')).toBeInTheDocument();
    expect(screen.getByText('Custom')).toBeInTheDocument();
    expect(screen.getByText(/SSO/)).toBeInTheDocument();
  });

  it('shows email input field', () => {
    render(<WaitlistSignup tier="team" />);
    expect(screen.getByPlaceholderText('Work email')).toBeInTheDocument();
  });

  it('stores signup in localStorage on submit', async () => {
    render(<WaitlistSignup tier="team" />);
    const emailInput = screen.getByPlaceholderText('Work email');
    fireEvent.change(emailInput, { target: { value: 'dev@company.com' } });
    fireEvent.submit(emailInput.closest('form')!);

    // Wait for state update
    await screen.findByText("You're on the list");

    const stored = JSON.parse(localStorage.getItem('4da_waitlist') || '[]');
    expect(stored).toHaveLength(1);
    expect(stored[0].email).toBe('dev@company.com');
    expect(stored[0].tier).toBe('team');
  });

  it('shows success state after submission', async () => {
    render(<WaitlistSignup tier="enterprise" />);
    const emailInput = screen.getByPlaceholderText('Work email');
    fireEvent.change(emailInput, { target: { value: 'cto@corp.com' } });
    fireEvent.submit(emailInput.closest('form')!);

    expect(await screen.findByText("You're on the list")).toBeInTheDocument();
    expect(screen.getByText(/Enterprise is available/)).toBeInTheDocument();
  });

  it('renders close button when onClose provided', () => {
    const onClose = vi.fn();
    render(<WaitlistSignup tier="team" onClose={onClose} />);
    const closeBtn = screen.getByLabelText('Close');
    expect(closeBtn).toBeInTheDocument();
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('disables submit when email is empty', () => {
    render(<WaitlistSignup tier="team" />);
    const submitBtn = screen.getByRole('button', { name: /Join Team Waitlist/ });
    expect(submitBtn).toBeDisabled();
  });
});
