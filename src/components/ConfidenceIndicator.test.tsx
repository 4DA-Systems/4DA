import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ConfidenceIndicator } from './ConfidenceIndicator';

describe('ConfidenceIndicator', () => {
  it('renders null when no confidence provided', () => {
    const { container } = render(<ConfidenceIndicator />);
    expect(container.firstChild).toBeNull();
  });

  it('displays high confidence (0.8+) with green styling', () => {
    render(<ConfidenceIndicator confidence={0.9} />);
    const indicator = screen.getByText(/±\d+%/);
    expect(indicator).toHaveClass('confidence-high');
  });

  it('displays medium confidence (0.5-0.8) with gray styling', () => {
    render(<ConfidenceIndicator confidence={0.6} />);
    const indicator = screen.getByText(/±\d+%/);
    expect(indicator).toHaveClass('confidence-medium');
  });

  it('displays low confidence (<0.5) with warning', () => {
    render(<ConfidenceIndicator confidence={0.3} />);
    expect(screen.getByText('results.lowConfidence')).toHaveClass('confidence-low');
  });

  it('calculates margin of error correctly', () => {
    render(<ConfidenceIndicator confidence={0.85} />);
    // 1 - 0.85 = 0.15 = 15%
    expect(screen.getByText(/±15%/)).toBeInTheDocument();
  });
});
