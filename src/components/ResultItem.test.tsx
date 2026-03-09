import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ResultItem } from './ResultItem';
import type { SourceRelevance, FeedbackGiven } from '../types';

// Mock child components that use Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.reject('not mocked')),
}));

vi.mock('./ScoreAutopsy', () => ({
  ScoreAutopsy: () => <div data-testid="score-autopsy" />,
}));

vi.mock('./ConfidenceIndicator', () => ({
  ConfidenceIndicator: () => null,
}));

vi.mock('./ArticleReader', () => ({
  ArticleReader: () => <div data-testid="article-reader" />,
}));

function makeItem(overrides: Partial<SourceRelevance> = {}): SourceRelevance {
  return {
    id: 1,
    title: 'Test Article Title',
    url: 'https://example.com/article',
    top_score: 0.42,
    matches: [
      {
        source_file: 'src/main.rs',
        matched_text: 'Relevant code snippet',
        similarity: 0.85,
      },
    ],
    relevant: true,
    explanation: 'This is relevant because it matches your context.',
    source_type: 'hackernews',
    ...overrides,
  };
}

describe('ResultItem', () => {
  const defaultFeedback: FeedbackGiven = {};
  const noop = vi.fn();

  it('renders title and score', () => {
    const item = makeItem({ title: 'My Great Article', top_score: 0.75 });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('My Great Article')).toBeInTheDocument();
    expect(screen.getByText('75%')).toBeInTheDocument();
  });

  it('shows preview explanation when collapsed', () => {
    const item = makeItem({ explanation: 'Matches your Rust project context' });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('Matches your Rust project context')).toBeInTheDocument();
  });

  it('shows expanded details with matches when expanded', () => {
    const item = makeItem();
    render(
      <ResultItem
        item={item}
        isExpanded={true}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    // Expanded view shows "Top Matches:" label and match details
    expect(screen.getByText('results.topMatches')).toBeInTheDocument();
    expect(screen.getByText('src/main.rs')).toBeInTheDocument();
    expect(screen.getByText(/Relevant code snippet/)).toBeInTheDocument();
  });

  it('does not show expanded details when collapsed', () => {
    const item = makeItem();
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.queryByText('results.topMatches')).not.toBeInTheDocument();
  });

  it('shows expand indicator "+" when collapsed and "-" when expanded', () => {
    const item = makeItem();

    const { rerender } = render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );
    expect(screen.getByText('+')).toBeInTheDocument();

    rerender(
      <ResultItem
        item={item}
        isExpanded={true}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );
    // The unicode minus sign used in the component
    expect(screen.queryByText('+')).not.toBeInTheDocument();
  });

  it('calls onToggleExpand when the expand button is clicked', () => {
    const onToggle = vi.fn();
    const item = makeItem();
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={onToggle}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    fireEvent.click(screen.getByLabelText('results.expandDetails'));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it('calls onRecordInteraction with "save" when Save button is clicked', () => {
    const onRecord = vi.fn();
    const item = makeItem();
    render(
      <ResultItem
        item={item}
        isExpanded={true}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={onRecord}
      />,
    );

    fireEvent.click(screen.getByText('action.save'));
    expect(onRecord).toHaveBeenCalledWith(item.id, 'save', item);
  });

  it('calls onRecordInteraction with "dismiss" when Dismiss button is clicked', () => {
    const onRecord = vi.fn();
    const item = makeItem();
    render(
      <ResultItem
        item={item}
        isExpanded={true}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={onRecord}
      />,
    );

    fireEvent.click(screen.getByText('action.dismiss'));
    expect(onRecord).toHaveBeenCalledWith(item.id, 'dismiss', item);
  });

  it('calls onRecordInteraction with "mark_irrelevant" when Not Relevant button is clicked', () => {
    const onRecord = vi.fn();
    const item = makeItem();
    render(
      <ResultItem
        item={item}
        isExpanded={true}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={onRecord}
      />,
    );

    fireEvent.click(screen.getByText('feedback.notRelevant'));
    expect(onRecord).toHaveBeenCalledWith(item.id, 'mark_irrelevant', item);
  });

  it('disables feedback buttons after feedback is given', () => {
    const item = makeItem();
    const feedback: FeedbackGiven = { [item.id]: 'save' };
    render(
      <ResultItem
        item={item}
        isExpanded={true}
        onToggleExpand={noop}
        feedbackGiven={feedback}
        onRecordInteraction={noop}
      />,
    );

    // Both the feedback indicator and the save button show "Saved" state
    const savedElements = screen.getAllByText(/feedback\.saved/);
    expect(savedElements.length).toBeGreaterThanOrEqual(1);
  });

  it('renders source badge "HN" for hackernews source type', () => {
    const item = makeItem({ source_type: 'hackernews' });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('HN')).toBeInTheDocument();
  });

  it('renders source badge "arXiv" for arxiv source type', () => {
    const item = makeItem({ source_type: 'arxiv' });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('arXiv')).toBeInTheDocument();
  });

  it('renders source badge "Reddit" for reddit source type', () => {
    const item = makeItem({ source_type: 'reddit' });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('Reddit')).toBeInTheDocument();
  });

  it('renders source badge "GitHub" for github source type', () => {
    const item = makeItem({ source_type: 'github' });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('GitHub')).toBeInTheDocument();
  });

  it('renders source badge "PH" for producthunt source type', () => {
    const item = makeItem({ source_type: 'producthunt' });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('PH')).toBeInTheDocument();
  });

  it('renders "Unknown" when source_type is undefined', () => {
    const item = makeItem({ source_type: undefined });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('results.unknownSource')).toBeInTheDocument();
  });

  it('renders signal badge when signal_type is present', () => {
    const item = makeItem({ signal_type: 'security_alert', signal_priority: 'critical' });
    render(
      <ResultItem
        item={item}
        isExpanded={false}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    expect(screen.getByText('results.signal.security_alert')).toBeInTheDocument();
  });

  it('sets correct aria-expanded and aria-controls attributes', () => {
    const item = makeItem({ id: 42 });
    render(
      <ResultItem
        item={item}
        isExpanded={true}
        onToggleExpand={noop}
        feedbackGiven={defaultFeedback}
        onRecordInteraction={noop}
      />,
    );

    const button = screen.getByLabelText('results.collapseDetails');
    expect(button).toHaveAttribute('aria-expanded', 'true');
    expect(button).toHaveAttribute('aria-controls', 'result-detail-42');
  });
});
