import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ContextualTip } from './ContextualTip';

describe('ContextualTip', () => {
  beforeEach(() => {
    // Clear localStorage before each test
    localStorage.clear();
  });

  it('renders the tip message when showWhen is true (default)', () => {
    render(<ContextualTip tipId="test-tip" message="Try this feature" />);
    expect(screen.getByText('Try this feature')).toBeInTheDocument();
  });

  it('renders the hint text when provided', () => {
    render(<ContextualTip tipId="test-tip" message="Main message" hint="Extra hint" />);
    expect(screen.getByText('Main message')).toBeInTheDocument();
    expect(screen.getByText('Extra hint')).toBeInTheDocument();
  });

  it('does not render when showWhen is false', () => {
    const { container } = render(
      <ContextualTip tipId="hidden-tip" message="Should not show" showWhen={false} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('dismisses when dismiss button is clicked', () => {
    render(<ContextualTip tipId="dismiss-tip" message="Dismissable tip" />);
    expect(screen.getByText('Dismissable tip')).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText('action.dismiss'));
    expect(screen.queryByText('Dismissable tip')).not.toBeInTheDocument();
  });

  it('saves dismissal to localStorage', () => {
    render(<ContextualTip tipId="persist-tip" message="Will be persisted" />);
    fireEvent.click(screen.getByLabelText('action.dismiss'));

    const stored = JSON.parse(localStorage.getItem('4da-dismissed-tips') || '[]');
    expect(stored).toContain('persist-tip');
  });

  it('does not render if tip was previously dismissed', () => {
    // Pre-dismiss the tip
    localStorage.setItem('4da-dismissed-tips', JSON.stringify(['pre-dismissed']));

    const { container } = render(
      <ContextualTip tipId="pre-dismissed" message="Should not show" />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders dismiss button with accessible label', () => {
    render(<ContextualTip tipId="a11y-tip" message="Accessible tip" />);
    expect(screen.getByLabelText('action.dismiss')).toBeInTheDocument();
  });
});
