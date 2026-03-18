import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Tauri API mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import SecurityDashboard from '../SecurityDashboard';

describe('SecurityDashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders severity count badges', () => {
    render(<SecurityDashboard />);
    expect(screen.getByText('Critical')).toBeInTheDocument();
    expect(screen.getByText('High')).toBeInTheDocument();
    expect(screen.getByText('Medium')).toBeInTheDocument();
    expect(screen.getByText('Low')).toBeInTheDocument();
  });

  it('renders active alerts with CVE identifiers', () => {
    render(<SecurityDashboard />);
    const cveElements = screen.getAllByText(/CVE-/);
    expect(cveElements.length).toBeGreaterThan(0);
  });

  it('renders Resolve buttons for active alerts', () => {
    render(<SecurityDashboard />);
    const resolveButtons = screen.getAllByText('Resolve');
    expect(resolveButtons.length).toBeGreaterThan(0);
  });

  it('renders resolved timeline section', () => {
    render(<SecurityDashboard />);
    expect(screen.getByText('Resolved')).toBeInTheDocument();
  });

  it('moves alert to resolved when Resolve is clicked', () => {
    render(<SecurityDashboard />);
    const resolveButtons = screen.getAllByText('Resolve');
    const initialCount = resolveButtons.length;
    fireEvent.click(resolveButtons[0]);
    const remaining = screen.queryAllByText('Resolve');
    expect(remaining.length).toBe(initialCount - 1);
  });
});
