import { describe, it, expect, vi } from 'vitest';
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

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(() => Promise.resolve(null)),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import IntelligenceConsole from '../IntelligenceConsole';

describe('IntelligenceConsole', () => {
  it('renders the header and subtitle', () => {
    render(<IntelligenceConsole />);
    expect(screen.getByText('Intelligence Console')).toBeInTheDocument();
    expect(
      screen.getByText('Accuracy tracking, project convergence, and AI cost analysis.'),
    ).toBeInTheDocument();
  });

  it('renders a tab bar with 3 tabs', () => {
    render(<IntelligenceConsole />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(3);
    expect(screen.getByText('Accuracy')).toBeInTheDocument();
    expect(screen.getByText('Projects')).toBeInTheDocument();
    expect(screen.getByText('AI Costs')).toBeInTheDocument();
  });

  it('defaults to the Accuracy tab being selected', () => {
    render(<IntelligenceConsole />);
    const accuracyTab = screen.getByRole('tab', { selected: true });
    expect(accuracyTab).toHaveTextContent('Accuracy');
  });

  it('switches to Projects tab on click', () => {
    render(<IntelligenceConsole />);
    const projectsTab = screen.getByText('Projects').closest('button')!;
    fireEvent.click(projectsTab);
    expect(projectsTab).toHaveAttribute('aria-selected', 'true');
  });

  it('switches to AI Costs tab on click', () => {
    render(<IntelligenceConsole />);
    const costsTab = screen.getByText('AI Costs').closest('button')!;
    fireEvent.click(costsTab);
    expect(costsTab).toHaveAttribute('aria-selected', 'true');
  });

  it('has a tabpanel for content', () => {
    render(<IntelligenceConsole />);
    expect(screen.getByRole('tabpanel')).toBeInTheDocument();
  });
});
