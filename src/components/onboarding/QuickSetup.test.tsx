import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QuickSetupStep } from './QuickSetupStep';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

// Mock Tauri event listener (used for ollama-pull-progress)
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock all child sub-components to isolate QuickSetupStep logic
vi.mock('./setup-ai-provider', () => ({
  SetupAIProvider: () => <div data-testid="setup-ai-provider">AI Provider Section</div>,
}));

vi.mock('./setup-projects', () => ({
  SetupProjects: () => <div data-testid="setup-projects">Projects Section</div>,
}));

vi.mock('./setup-stack', () => ({
  SetupStack: () => <div data-testid="setup-stack">Stack Section</div>,
}));

vi.mock('./setup-interests', () => ({
  SetupInterests: () => <div data-testid="setup-interests">Interests Section</div>,
}));

vi.mock('./setup-locale', () => ({
  SetupLocale: () => <div data-testid="setup-locale">Locale Section</div>,
}));

describe('QuickSetupStep', () => {
  const defaultProps = {
    isAnimating: false,
    onComplete: vi.fn(),
    onBack: vi.fn(),
  };

  it('renders the setup title and subtitle', () => {
    render(<QuickSetupStep {...defaultProps} />);
    expect(screen.getByText('onboarding.setup.title')).toBeInTheDocument();
    expect(screen.getByText('onboarding.setup.subtitle')).toBeInTheDocument();
  });

  it('renders all section headers', () => {
    render(<QuickSetupStep {...defaultProps} />);
    expect(screen.getByText('onboarding.setup.aiProvider')).toBeInTheDocument();
    expect(screen.getByText('onboarding.setup.yourProjects')).toBeInTheDocument();
    expect(screen.getByText('onboarding.setup.yourStack')).toBeInTheDocument();
    expect(screen.getByText('onboarding.setup.yourRegion')).toBeInTheDocument();
    expect(screen.getByText('onboarding.setup.yourInterests')).toBeInTheDocument();
  });

  it('renders the back button', () => {
    render(<QuickSetupStep {...defaultProps} />);
    expect(screen.getByLabelText('onboarding.setup.goBack')).toBeInTheDocument();
  });

  it('calls onBack when the back button is clicked', () => {
    const onBack = vi.fn();
    render(<QuickSetupStep {...defaultProps} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText('onboarding.setup.goBack'));
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('renders the complete/enter button', () => {
    render(<QuickSetupStep {...defaultProps} />);
    expect(screen.getByLabelText('onboarding.setup.completeSetup')).toBeInTheDocument();
    expect(screen.getByText('onboarding.setup.enter4DA')).toBeInTheDocument();
  });

  it('shows "all sections optional" hint text', () => {
    render(<QuickSetupStep {...defaultProps} />);
    expect(screen.getByText('onboarding.setup.allSectionsOptional')).toBeInTheDocument();
  });

  it('section headers have aria-expanded attribute', () => {
    render(<QuickSetupStep {...defaultProps} />);
    // AI Provider section should start expanded (aiOpen = true)
    const aiButton = screen.getByText('onboarding.setup.aiProvider').closest('button');
    expect(aiButton).toHaveAttribute('aria-expanded', 'true');
  });

  it('section headers are clickable buttons', () => {
    render(<QuickSetupStep {...defaultProps} />);

    // All section headers should be rendered as buttons
    const aiButton = screen.getByText('onboarding.setup.aiProvider').closest('button');
    const projectsButton = screen.getByText('onboarding.setup.yourProjects').closest('button');
    const stackButton = screen.getByText('onboarding.setup.yourStack').closest('button');
    const regionButton = screen.getByText('onboarding.setup.yourRegion').closest('button');
    const interestsButton = screen.getByText('onboarding.setup.yourInterests').closest('button');

    expect(aiButton).toBeInTheDocument();
    expect(projectsButton).toBeInTheDocument();
    expect(stackButton).toBeInTheDocument();
    expect(regionButton).toBeInTheDocument();
    expect(interestsButton).toBeInTheDocument();

    // All should have aria-expanded attribute (value may vary based on async effects)
    expect(aiButton).toHaveAttribute('aria-expanded');
    expect(projectsButton).toHaveAttribute('aria-expanded');
    expect(stackButton).toHaveAttribute('aria-expanded');
    expect(regionButton).toHaveAttribute('aria-expanded');
    expect(interestsButton).toHaveAttribute('aria-expanded');

    // Clicking should not throw
    fireEvent.click(projectsButton!);
    fireEvent.click(stackButton!);
  });

  it('renders the AI Provider child component when section is open', () => {
    render(<QuickSetupStep {...defaultProps} />);
    // AI Provider section is open by default
    expect(screen.getByTestId('setup-ai-provider')).toBeInTheDocument();
  });

  it('applies opacity transition classes based on isAnimating prop', () => {
    const { container, rerender } = render(<QuickSetupStep {...defaultProps} isAnimating={false} />);
    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper.className).toContain('opacity-100');

    rerender(<QuickSetupStep {...defaultProps} isAnimating={true} />);
    const wrapperAnimating = container.firstElementChild as HTMLElement;
    expect(wrapperAnimating.className).toContain('opacity-0');
  });

  it('does not show error alert initially', () => {
    render(<QuickSetupStep {...defaultProps} />);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
