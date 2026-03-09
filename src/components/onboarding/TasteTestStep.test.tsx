import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';

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
// Child component mocks
// ---------------------------------------------------------------------------
vi.mock('./TasteTestCard', () => ({
  TasteTestCard: ({ card, onInterested, onSkip, onStrongInterest, isAnimating }: {
    card: { title: string; snippet: string; sourceHint: string; categoryHint: string };
    onInterested: () => void;
    onSkip: () => void;
    onStrongInterest: () => void;
    isAnimating: boolean;
  }) => (
    <div data-testid="taste-test-card" data-animating={isAnimating}>
      <span data-testid="card-title">{card.title}</span>
      <span data-testid="card-snippet">{card.snippet}</span>
      <span data-testid="card-source">{card.sourceHint}</span>
      <span data-testid="card-category">{card.categoryHint}</span>
      <button data-testid="interested-btn" onClick={onInterested}>Interested</button>
      <button data-testid="skip-card-btn" onClick={onSkip}>Skip Card</button>
      <button data-testid="strong-interest-btn" onClick={onStrongInterest}>Love</button>
    </div>
  ),
}));

vi.mock('./CalibrationSummary', () => ({
  CalibrationSummary: ({ summary, onContinue }: {
    summary: { dominantPersonaName: string; confidence: number };
    onContinue: () => void;
  }) => (
    <div data-testid="calibration-summary">
      <span data-testid="persona-name">{summary.dominantPersonaName}</span>
      <span data-testid="confidence-value">{Math.round(summary.confidence * 100)}%</span>
      <button data-testid="continue-btn" onClick={onContinue}>Continue</button>
    </div>
  ),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { TasteTestStep } from './TasteTestStep';

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------
const mockCard = {
  id: 1,
  slot: 0,
  title: 'Understanding Rust Lifetimes',
  snippet: 'A deep dive into Rust ownership and borrowing.',
  sourceHint: 'Hacker News',
  categoryHint: 'Systems Programming',
};

const mockNextCardResult = {
  type: 'nextCard' as const,
  card: mockCard,
  progress: 0.2,
  confidence: 0.35,
};

const mockCompleteResult = {
  type: 'complete' as const,
  summary: {
    dominantPersonaName: 'Systems Architect',
    dominantPersonaDescription: 'You gravitate toward low-level systems.',
    confidence: 0.85,
    itemsShown: 10,
    personaWeights: [
      { name: 'Systems Architect', weight: 0.6 },
      { name: 'Full-Stack Builder', weight: 0.4 },
    ],
    topInterests: ['Rust', 'concurrency', 'performance'],
  },
};

describe('TasteTestStep', () => {
  const mockOnComplete = vi.fn();
  const mockOnSkip = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    mockOnComplete.mockClear();
    mockOnSkip.mockClear();
    vi.mocked(invoke).mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // -------------------------------------------------------------------------
  // 1. Renders without crash — intro phase
  // -------------------------------------------------------------------------
  it('renders without crash in intro phase', () => {
    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    expect(screen.getByText("Let's calibrate your feed")).toBeInTheDocument();
    expect(screen.getByText('Start calibration')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 2. Shows intro description text
  // -------------------------------------------------------------------------
  it('shows calibration description text', () => {
    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    expect(
      screen.getByText(/We'll show you up to 15 articles/),
    ).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 3. Start calibration transitions to cards phase
  // -------------------------------------------------------------------------
  it('shows calibration cards after clicking Start calibration', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(mockNextCardResult);

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    expect(invoke).toHaveBeenCalledWith('taste_test_start', {});
    expect(screen.getByTestId('taste-test-card')).toBeInTheDocument();
    expect(screen.getByTestId('card-title')).toHaveTextContent('Understanding Rust Lifetimes');
  });

  // -------------------------------------------------------------------------
  // 4. Shows confidence percentage during cards phase
  // -------------------------------------------------------------------------
  it('shows confidence percentage in cards phase', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(mockNextCardResult);

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    expect(screen.getByText('35% confident')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 5. Handles interested response — advances to next card
  // -------------------------------------------------------------------------
  it('advances to next card when interested is clicked', async () => {
    const secondCard = {
      ...mockCard,
      id: 2,
      slot: 1,
      title: 'React Server Components Deep Dive',
    };

    vi.mocked(invoke)
      .mockResolvedValueOnce(mockNextCardResult) // taste_test_start
      .mockResolvedValueOnce({
        type: 'nextCard',
        card: secondCard,
        progress: 0.4,
        confidence: 0.55,
      }); // taste_test_respond

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    // Start the test
    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    // Click interested and flush the 150ms animation delay + async invoke
    await act(async () => {
      fireEvent.click(screen.getByTestId('interested-btn'));
      await vi.runAllTimersAsync();
    });

    expect(invoke).toHaveBeenCalledWith('taste_test_respond', expect.objectContaining({
      itemSlot: 0,
      response: 'interested',
    }));

    expect(screen.getByTestId('card-title')).toHaveTextContent('React Server Components Deep Dive');
    expect(screen.getByText('55% confident')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 6. Handles skip response on a card (not_interested)
  // -------------------------------------------------------------------------
  it('sends not_interested when skip card is clicked', async () => {
    const secondCard = { ...mockCard, id: 2, slot: 1, title: 'Another Article' };
    vi.mocked(invoke)
      .mockResolvedValueOnce(mockNextCardResult)
      .mockResolvedValueOnce({
        type: 'nextCard',
        card: secondCard,
        progress: 0.4,
        confidence: 0.45,
      });

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId('skip-card-btn'));
      await vi.runAllTimersAsync();
    });

    expect(invoke).toHaveBeenCalledWith('taste_test_respond', expect.objectContaining({
      response: 'not_interested',
    }));
  });

  // -------------------------------------------------------------------------
  // 7. Skip button in intro phase calls onSkip
  // -------------------------------------------------------------------------
  it('calls onSkip when skip button is clicked in intro phase', () => {
    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    fireEvent.click(screen.getByText('Skip for now'));
    expect(mockOnSkip).toHaveBeenCalledTimes(1);
  });

  // -------------------------------------------------------------------------
  // 8. Skip calibration button in cards phase calls onSkip
  // -------------------------------------------------------------------------
  it('calls onSkip when skip calibration is clicked during cards phase', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(mockNextCardResult);

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    fireEvent.click(screen.getByText('Skip calibration'));
    expect(mockOnSkip).toHaveBeenCalledTimes(1);
  });

  // -------------------------------------------------------------------------
  // 9. Completes test and shows calibration summary
  // -------------------------------------------------------------------------
  it('shows calibration summary when test is complete', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(mockNextCardResult) // taste_test_start
      .mockResolvedValueOnce(mockCompleteResult) // taste_test_respond -> complete
      .mockResolvedValueOnce(mockCompleteResult.summary); // taste_test_finalize

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    // Start calibration
    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    // Respond to trigger completion — flush animation delay + async invoke chain
    await act(async () => {
      fireEvent.click(screen.getByTestId('interested-btn'));
      await vi.runAllTimersAsync();
    });

    expect(invoke).toHaveBeenCalledWith('taste_test_finalize', {});
    expect(screen.getByTestId('calibration-summary')).toBeInTheDocument();
    expect(screen.getByTestId('persona-name')).toHaveTextContent('Systems Architect');
  });

  // -------------------------------------------------------------------------
  // 10. CalibrationSummary continue calls onComplete
  // -------------------------------------------------------------------------
  it('calls onComplete when continue is clicked on summary', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(mockNextCardResult)
      .mockResolvedValueOnce(mockCompleteResult)
      .mockResolvedValueOnce(mockCompleteResult.summary);

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId('interested-btn'));
      await vi.runAllTimersAsync();
    });

    expect(screen.getByTestId('calibration-summary')).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('continue-btn'));
    expect(mockOnComplete).toHaveBeenCalledTimes(1);
  });

  // -------------------------------------------------------------------------
  // 11. Shows error when taste_test_start fails
  // -------------------------------------------------------------------------
  it('shows error message when start fails', async () => {
    vi.mocked(invoke).mockRejectedValueOnce('Network error');

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    expect(screen.getByText(/Failed to start taste test/)).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 12. Shows "Starting..." while loading
  // -------------------------------------------------------------------------
  it('shows Starting... text while waiting for taste_test_start', async () => {
    // Never resolve to keep in loading state
    vi.mocked(invoke).mockReturnValueOnce(new Promise(() => {}));

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
    });

    expect(screen.getByText('Starting...')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 13. Applies opacity animation class when isAnimating is true
  // -------------------------------------------------------------------------
  it('applies opacity-0 class when isAnimating prop is true', () => {
    const { container } = render(
      <TasteTestStep isAnimating={true} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).toContain('opacity-0');
  });

  // -------------------------------------------------------------------------
  // 14. Shows finalizing spinner
  // -------------------------------------------------------------------------
  it('shows analyzing text during finalizing phase', async () => {
    // taste_test_start returns a card
    vi.mocked(invoke).mockResolvedValueOnce(mockNextCardResult);
    // taste_test_respond returns complete
    vi.mocked(invoke).mockResolvedValueOnce(mockCompleteResult);
    // taste_test_finalize never resolves to stay in finalizing state
    vi.mocked(invoke).mockReturnValueOnce(new Promise(() => {}));

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId('interested-btn'));
      await vi.runAllTimersAsync();
    });

    expect(screen.getByText('Analyzing your preferences...')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 15. Strong interest sends strong_interest response
  // -------------------------------------------------------------------------
  it('sends strong_interest response when love button is clicked', async () => {
    const secondCard = { ...mockCard, id: 2, slot: 1, title: 'Next Article' };
    vi.mocked(invoke)
      .mockResolvedValueOnce(mockNextCardResult)
      .mockResolvedValueOnce({
        type: 'nextCard',
        card: secondCard,
        progress: 0.4,
        confidence: 0.6,
      });

    render(
      <TasteTestStep isAnimating={false} onComplete={mockOnComplete} onSkip={mockOnSkip} />,
    );

    await act(async () => {
      fireEvent.click(screen.getByText('Start calibration'));
      await vi.runAllTimersAsync();
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId('strong-interest-btn'));
      await vi.runAllTimersAsync();
    });

    expect(invoke).toHaveBeenCalledWith('taste_test_respond', expect.objectContaining({
      response: 'strong_interest',
    }));
  });
});
