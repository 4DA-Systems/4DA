import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

// Configurable mock state
let mockState: Record<string, unknown> = {};
function setMockState(overrides: Record<string, unknown>) {
  mockState = {
    feedbackGiven: {},
    lastLearnedTopic: null,
    ...overrides,
  };
}

vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

import { LearningBadge } from './LearningBadge';

describe('LearningBadge', () => {
  it('returns null when feedbackCount is 0', () => {
    setMockState({ feedbackGiven: {} });
    const { container } = render(<LearningBadge />);
    expect(container.innerHTML).toBe('');
  });

  it('renders learned count when feedback exists', () => {
    setMockState({
      feedbackGiven: { '1': 'save', '2': 'dismiss', '3': 'save' },
    });
    render(<LearningBadge />);
    expect(screen.getByText('header.signalsLearned')).toBeInTheDocument();
  });

  it('renders the pulsing indicator dot', () => {
    setMockState({
      feedbackGiven: { '1': 'save' },
    });
    const { container } = render(<LearningBadge />);
    const dot = container.querySelector('.animate-pulse');
    expect(dot).toBeInTheDocument();
  });

  it('shows the learned topic when lastLearnedTopic is recent and positive', () => {
    setMockState({
      feedbackGiven: { '1': 'save' },
      lastLearnedTopic: {
        topic: 'Rust',
        direction: 'positive',
        timestamp: Date.now(),
      },
    });
    render(<LearningBadge />);
    expect(screen.getByText(/\+ Rust/)).toBeInTheDocument();
  });

  it('shows the learned topic with minus when direction is negative', () => {
    setMockState({
      feedbackGiven: { '1': 'dismiss' },
      lastLearnedTopic: {
        topic: 'Blockchain',
        direction: 'negative',
        timestamp: Date.now(),
      },
    });
    render(<LearningBadge />);
    expect(screen.getByText(/- Blockchain/)).toBeInTheDocument();
  });

  it('shows count text when lastLearnedTopic is old', () => {
    setMockState({
      feedbackGiven: { '1': 'save', '2': 'save' },
      lastLearnedTopic: {
        topic: 'Old Topic',
        direction: 'positive',
        timestamp: Date.now() - 10000, // 10 seconds ago — past the 3s window
      },
    });
    render(<LearningBadge />);
    // Should show the count text, not the topic
    expect(screen.getByText('header.signalsLearned')).toBeInTheDocument();
  });
});
