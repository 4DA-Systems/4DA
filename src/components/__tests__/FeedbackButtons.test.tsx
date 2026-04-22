// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
import { FeedbackButtons } from '../result-item/FeedbackButtons';
import { makeItem } from '../../test/factories';
import type { SourceRelevance, FeedbackAction } from '../../types';

describe('FeedbackButtons', () => {
  let mockOnRecordInteraction: ReturnType<typeof vi.fn>;
  let defaultItem: SourceRelevance;

  beforeEach(() => {
    vi.clearAllMocks();
    mockOnRecordInteraction = vi.fn();
    defaultItem = makeItem();
  });

  it('renders without crash', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    expect(screen.getByRole('group')).toBeInTheDocument();
  });

  it('shows Open Link button when item has URL', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    expect(screen.getByText('feedback.openLink')).toBeInTheDocument();
  });

  it('hides Open Link button when item has no URL', () => {
    const itemNoUrl = makeItem({ url: undefined });
    render(
      <FeedbackButtons
        item={itemNoUrl}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    expect(screen.queryByText('feedback.openLink')).not.toBeInTheDocument();
  });

  it('shows Save button', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    expect(screen.getByText('action.save')).toBeInTheDocument();
  });

  it('shows Dismiss button in overflow menu', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    fireEvent.click(screen.getByLabelText('feedback.moreActions'));
    expect(screen.getByText('action.dismiss')).toBeInTheDocument();
  });

  it('shows Not Relevant button', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    expect(screen.getByText('feedback.notRelevant')).toBeInTheDocument();
  });

  it('calls onRecordInteraction with "save" when Save is clicked', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    fireEvent.click(screen.getByText('action.save'));
    expect(mockOnRecordInteraction).toHaveBeenCalledWith(defaultItem.id, 'save', defaultItem);
  });

  it('calls onRecordInteraction with "dismiss" when Dismiss is clicked', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    fireEvent.click(screen.getByLabelText('feedback.moreActions'));
    fireEvent.click(screen.getByText('action.dismiss'));
    expect(mockOnRecordInteraction).toHaveBeenCalledWith(defaultItem.id, 'dismiss', defaultItem);
  });

  it('calls onRecordInteraction with "mark_irrelevant" when Not Relevant is clicked', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    fireEvent.click(screen.getByText('feedback.notRelevant'));
    expect(mockOnRecordInteraction).toHaveBeenCalledWith(defaultItem.id, 'mark_irrelevant', defaultItem);
  });

  it('calls onRecordInteraction with "click" when Open Link is clicked', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    fireEvent.click(screen.getByText('feedback.openLink'));
    expect(mockOnRecordInteraction).toHaveBeenCalledWith(defaultItem.id, 'click', defaultItem);
  });

  it('shows saved state after save feedback', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={'save' as FeedbackAction}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    expect(screen.getByText(/feedback\.saved/)).toBeInTheDocument();
  });

  it('shows dismissed state after dismiss feedback', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={'dismiss' as FeedbackAction}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    fireEvent.click(screen.getByLabelText('feedback.moreActions'));
    expect(screen.getByText(/feedback\.dismissed/)).toBeInTheDocument();
  });

  it('shows marked state after mark_irrelevant feedback', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={'mark_irrelevant' as FeedbackAction}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    expect(screen.getByText(/feedback\.marked/)).toBeInTheDocument();
  });

  it('disables feedback action buttons after any feedback is given', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={'save' as FeedbackAction}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    // Primary feedback buttons (Save, Snooze, Not Relevant) should be disabled.
    // Open Link and the overflow trigger (···) remain enabled.
    const allButtons = screen.getAllByRole('button');
    const alwaysEnabled = new Set(['feedback.openLink', '···']);
    const feedbackButtons = allButtons.filter(
      (btn) => !alwaysEnabled.has(btn.textContent ?? ''),
    );
    feedbackButtons.forEach((btn) => {
      expect(btn).toBeDisabled();
    });
  });

  it('does not call onRecordInteraction when buttons are disabled', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={'save' as FeedbackAction}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    // Try clicking a disabled button (dismiss is now in overflow)
    fireEvent.click(screen.getByLabelText('feedback.moreActions'));
    const dismissBtn = screen.getByText('action.dismiss');
    fireEvent.click(dismissBtn);
    expect(mockOnRecordInteraction).not.toHaveBeenCalled();
  });

  it('Open Link is a button that uses Tauri opener plugin', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    const link = screen.getByText('feedback.openLink');
    // Open Link is now a <button> that calls @tauri-apps/plugin-opener instead of an <a> tag
    expect(link.tagName).toBe('BUTTON');
  });

  it('has accessible group label', () => {
    render(
      <FeedbackButtons
        item={defaultItem}
        feedback={undefined}
        onRecordInteraction={mockOnRecordInteraction}
      />,
    );
    expect(screen.getByRole('group')).toHaveAttribute('aria-label', 'feedback.actions');
  });
});
