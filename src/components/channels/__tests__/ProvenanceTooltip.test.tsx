import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.reject('not mocked')),
}));

import { ProvenanceTooltip } from '../ProvenanceTooltip';

const mockProvenance = {
  render_id: 1,
  claim_index: 0,
  claim_text: 'Tauri 2.0 brings major improvements',
  source_item_ids: [101, 102],
  source_titles: ['Tauri 2.0 Release Notes', 'Rust Security Advisory'],
  source_urls: ['https://tauri.app/blog/2.0', 'https://rustsec.org/advisory-1'],
};

describe('ProvenanceTooltip', () => {
  it('renders trigger button with children', () => {
    render(
      <ProvenanceTooltip provenance={mockProvenance}>
        Click me
      </ProvenanceTooltip>,
    );
    expect(screen.getByText('Click me')).toBeInTheDocument();
  });

  it('trigger button has aria-expanded and aria-haspopup', () => {
    render(
      <ProvenanceTooltip provenance={mockProvenance}>
        Trigger
      </ProvenanceTooltip>,
    );
    const btn = screen.getByRole('button');
    expect(btn).toHaveAttribute('aria-expanded', 'false');
    expect(btn).toHaveAttribute('aria-haspopup', 'true');
  });

  it('opens tooltip on click showing provenance label', () => {
    render(
      <ProvenanceTooltip provenance={mockProvenance}>
        Trigger
      </ProvenanceTooltip>,
    );
    fireEvent.click(screen.getByRole('button'));

    expect(screen.getByText('channels.provenance')).toBeInTheDocument();
    expect(screen.getByRole('button')).toHaveAttribute('aria-expanded', 'true');
  });

  it('shows source titles when open', () => {
    render(
      <ProvenanceTooltip provenance={mockProvenance}>
        Trigger
      </ProvenanceTooltip>,
    );
    fireEvent.click(screen.getByRole('button'));

    expect(screen.getByText('Tauri 2.0 Release Notes')).toBeInTheDocument();
    expect(screen.getByText('Rust Security Advisory')).toBeInTheDocument();
  });

  it('shows source URLs as links', () => {
    render(
      <ProvenanceTooltip provenance={mockProvenance}>
        Trigger
      </ProvenanceTooltip>,
    );
    fireEvent.click(screen.getByRole('button'));

    const links = screen.getAllByRole('link');
    expect(links).toHaveLength(2);
    expect(links[0]).toHaveAttribute('href', 'https://tauri.app/blog/2.0');
    expect(links[1]).toHaveAttribute('href', 'https://rustsec.org/advisory-1');
    // Security: external links have noopener noreferrer
    expect(links[0]).toHaveAttribute('rel', 'noopener noreferrer');
  });

  it('closes on Escape key', () => {
    render(
      <ProvenanceTooltip provenance={mockProvenance}>
        Trigger
      </ProvenanceTooltip>,
    );
    fireEvent.click(screen.getByRole('button'));
    expect(screen.getByText('channels.provenance')).toBeInTheDocument();

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByText('channels.provenance')).not.toBeInTheDocument();
  });

  it('tooltip has role="tooltip"', () => {
    render(
      <ProvenanceTooltip provenance={mockProvenance}>
        Trigger
      </ProvenanceTooltip>,
    );
    fireEvent.click(screen.getByRole('button'));
    expect(screen.getByRole('tooltip')).toBeInTheDocument();
  });
});
