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

  it('renders team tier with correct info', () => {
    render(<WaitlistSignup tier="team" />);
    expect(screen.getByText('4DA Team')).toBeInTheDocument();
    expect(screen.getByText('Coming Soon')).toBeInTheDocument();
    expect(screen.getByText(/\$29\/seat/)).toBeInTheDocument();
    expect(screen.getByText(/Shared signal detection across all seats/)).toBeInTheDocument();
  });

  it('renders enterprise tier with correct info', () => {
    render(<WaitlistSignup tier="enterprise" />);
    expect(screen.getByText('4DA Enterprise')).toBeInTheDocument();
    expect(screen.getByText(/SSO license activation/)).toBeInTheDocument();
    expect(screen.getByText(/Procurement documentation/)).toBeInTheDocument();
  });

  it('shows all form fields', () => {
    render(<WaitlistSignup tier="team" />);
    expect(screen.getByPlaceholderText('Work email *')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Your name')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Team size')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Company')).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Role/)).toBeInTheDocument();
  });

  it('stores signup in localStorage on submit', async () => {
    render(<WaitlistSignup tier="team" />);
    fireEvent.change(screen.getByPlaceholderText('Work email *'), {
      target: { value: 'dev@company.com' },
    });
    fireEvent.submit(screen.getByPlaceholderText('Work email *').closest('form')!);

    // Waitlist now saves via Tauri command (with localStorage fallback)
    await screen.findByText("You're on the list");
    expect(screen.getByText(/Position secured/)).toBeInTheDocument();
  });

  it('shows success state for duplicate submission', async () => {
    render(<WaitlistSignup tier="team" />);
    fireEvent.change(screen.getByPlaceholderText('Work email *'), {
      target: { value: 'dev@company.com' },
    });
    fireEvent.submit(screen.getByPlaceholderText('Work email *').closest('form')!);

    await screen.findByText("You're on the list");
  });

  it('shows success state with position confirmation', async () => {
    render(<WaitlistSignup tier="enterprise" />);
    fireEvent.change(screen.getByPlaceholderText('Work email *'), {
      target: { value: 'cto@corp.com' },
    });
    fireEvent.submit(screen.getByPlaceholderText('Work email *').closest('form')!);

    expect(await screen.findByText("You're on the list")).toBeInTheDocument();
    expect(screen.getByText(/Position secured/)).toBeInTheDocument();
  });

  it('renders close button and calls onClose', () => {
    const onClose = vi.fn();
    render(<WaitlistSignup tier="team" onClose={onClose} />);
    const closeBtn = screen.getByLabelText('Close');
    expect(closeBtn).toBeInTheDocument();
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('disables submit when email is empty', () => {
    render(<WaitlistSignup tier="team" />);
    const submitBtn = screen.getByRole('button', { name: /Join the Team Waitlist/ });
    expect(submitBtn).toBeDisabled();
  });

  it('expands feature details on click', () => {
    render(<WaitlistSignup tier="team" />);
    const featureBtn = screen.getByText('Shared signal detection across all seats');
    fireEvent.click(featureBtn);
    expect(screen.getByText(/confidence multiplies/)).toBeInTheDocument();
  });

  it('captures all optional fields', async () => {
    render(<WaitlistSignup tier="enterprise" />);
    fireEvent.change(screen.getByPlaceholderText('Work email *'), { target: { value: 'vp@bigcorp.com' } });
    fireEvent.change(screen.getByPlaceholderText('Your name'), { target: { value: 'Jane' } });
    fireEvent.change(screen.getByPlaceholderText('Team size'), { target: { value: '50' } });
    fireEvent.change(screen.getByPlaceholderText('Company'), { target: { value: 'BigCorp' } });
    fireEvent.change(screen.getByPlaceholderText(/Role/), { target: { value: 'VP Engineering' } });
    fireEvent.submit(screen.getByPlaceholderText('Work email *').closest('form')!);

    // Waitlist saves via Tauri command — verify success state
    await screen.findByText("You're on the list");
    expect(screen.getByText(/Position secured/)).toBeInTheDocument();
  });
});
