import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { SmartEmptyState } from '../SmartEmptyState';

describe('SmartEmptyState', () => {
  it('renders example signals for React stack', () => {
    render(<SmartEmptyState detectedStack={['react', 'typescript']} />);
    expect(screen.getAllByText(/React/).length).toBeGreaterThan(0);
    expect(screen.getAllByText('empty.example').length).toBeGreaterThan(0);
  });

  it('renders example signals for Rust stack', () => {
    render(<SmartEmptyState detectedStack={['rust', 'cargo']} />);
    expect(screen.getByText(/Tokio 2\.0/)).toBeInTheDocument();
  });

  it('renders default signals when stack is empty', () => {
    render(<SmartEmptyState detectedStack={[]} />);
    expect(screen.getByText('empty.whileAnalysisRuns')).toBeInTheDocument();
  });

  it('shows header with detected stack name', () => {
    render(<SmartEmptyState detectedStack={['react']} />);
    expect(screen.getByText('empty.whileAnalysisRunsStack')).toBeInTheDocument();
  });

  it('shows footer with arrival estimate', () => {
    render(<SmartEmptyState detectedStack={['react']} />);
    expect(screen.getByText('empty.realSignalsArriving')).toBeInTheDocument();
  });
});
