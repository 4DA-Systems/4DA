// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ConfidenceIndicator } from './ConfidenceIndicator';

describe('ConfidenceIndicator', () => {
  it('renders null when no confidence provided', () => {
    const { container } = render(<ConfidenceIndicator />);
    expect(container.firstChild).toBeNull();
  });

  it('displays high confidence (0.8+) with a qualitative label, not a fabricated ±%', () => {
    render(<ConfidenceIndicator confidence={0.9} />);
    const indicator = screen.getByText('results.highConfidence');
    expect(indicator).toHaveClass('confidence-high');
  });

  it('displays medium confidence (0.5-0.8) with a qualitative label', () => {
    render(<ConfidenceIndicator confidence={0.6} />);
    const indicator = screen.getByText('results.mediumConfidence');
    expect(indicator).toHaveClass('confidence-medium');
  });

  it('displays low confidence (<0.5) with a qualitative label', () => {
    render(<ConfidenceIndicator confidence={0.3} />);
    expect(screen.getByText('results.lowConfidence')).toHaveClass('confidence-low');
  });

  it('never renders a fabricated "±N%" margin of error for any confidence', () => {
    // The model emits a single confidence scalar; presenting `±(1-conf)%` dressed
    // it up as a statistical interval the system never computed (banned fabricated
    // precision). Guard the contract: no ± at any confidence level.
    for (const c of [0.3, 0.6, 0.85, 0.92, 0.99]) {
      const { container, unmount } = render(<ConfidenceIndicator confidence={c} />);
      expect(container.textContent).not.toMatch(/±/);
      unmount();
    }
  });

  it('shows signal concordance when signalCount provided', () => {
    render(<ConfidenceIndicator signalCount={3} confirmedSignals={['context', 'interest', 'dependency']} />);
    const indicator = screen.getByText(/signalConcordance/);
    expect(indicator).toHaveClass('confidence-medium');
    expect(indicator).toHaveAttribute('title', 'context, interest, dependency');
  });

  it('shows high concordance for 4+ signals', () => {
    render(<ConfidenceIndicator signalCount={4} confirmedSignals={['context', 'interest', 'ace', 'dependency']} />);
    const indicator = screen.getByText(/signalConcordance/);
    expect(indicator).toHaveClass('confidence-high');
  });

  it('shows low concordance for 0-1 signals', () => {
    render(<ConfidenceIndicator signalCount={1} confirmedSignals={['interest']} />);
    const indicator = screen.getByText(/signalConcordance/);
    expect(indicator).toHaveClass('confidence-low');
  });

  it('prefers signal concordance over confidence number', () => {
    render(<ConfidenceIndicator confidence={0.9} signalCount={2} confirmedSignals={['context', 'interest']} />);
    expect(screen.getByText(/signalConcordance/)).toBeInTheDocument();
    expect(screen.queryByText(/±/)).toBeNull();
  });
});
