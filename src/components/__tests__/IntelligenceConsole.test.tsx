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
    expect(screen.getByText('console.title')).toBeInTheDocument();
    expect(
      screen.getByText('console.subtitle'),
    ).toBeInTheDocument();
  });

  it('renders a tab bar with 4 tabs', () => {
    render(<IntelligenceConsole />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(4);
    expect(screen.getByText('console.tab_accuracy')).toBeInTheDocument();
    expect(screen.getByText('console.tab_convergence')).toBeInTheDocument();
    expect(screen.getByText('console.tab_costs')).toBeInTheDocument();
    expect(screen.getByText('console.tab_wisdom')).toBeInTheDocument();
  });

  it('defaults to the Accuracy tab being selected', () => {
    render(<IntelligenceConsole />);
    const accuracyTab = screen.getByRole('tab', { selected: true });
    expect(accuracyTab).toHaveTextContent('console.tab_accuracy');
  });

  it('switches to Projects tab on click', () => {
    render(<IntelligenceConsole />);
    const projectsTab = screen.getByText('console.tab_convergence').closest('button')!;
    fireEvent.click(projectsTab);
    expect(projectsTab).toHaveAttribute('aria-selected', 'true');
  });

  it('switches to AI Costs tab on click', () => {
    render(<IntelligenceConsole />);
    const costsTab = screen.getByText('console.tab_costs').closest('button')!;
    fireEvent.click(costsTab);
    expect(costsTab).toHaveAttribute('aria-selected', 'true');
  });

  it('has a tabpanel for content', () => {
    render(<IntelligenceConsole />);
    expect(screen.getByRole('tabpanel')).toBeInTheDocument();
  });
});
