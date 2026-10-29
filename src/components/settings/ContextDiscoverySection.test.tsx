// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import { ContextDiscoverySection } from './ContextDiscoverySection';

function renderWith(excluded: string[]) {
  return render(
    <ContextDiscoverySection
      scanDirectories={['D:\\4DA', 'C:\\Users\\me\\Documents\\kairos-mvp', '/home/me/projects/4da']}
      missingScanDirectories={['/home/me/projects/4da']}
      excludedScanDirectories={excluded}
      newScanDir=""
      setNewScanDir={vi.fn()}
      isScanning={false}
      discoveredContext={{ tech: [], topics: [], lastScan: null }}
      runAutoDiscovery={vi.fn()}
      runFullScan={vi.fn()}
      addScanDirectory={vi.fn()}
      removeScanDirectory={vi.fn()}
    />,
  );
}

// Audit L7 (2026-10-12): Settings > Projects listed three Documents roots that
// Your Stack excluded, with nothing to say so, while the lockfile walk skipped
// them "excluded by the user (Your Stack)".
describe('ContextDiscoverySection — roots excluded in Your Stack', () => {
  it('marks exactly the excluded root, with the reason on hover', () => {
    renderWith(['C:\\Users\\me\\Documents\\kairos-mvp']);
    const markers = screen.getAllByTestId('scan-dir-excluded');
    expect(markers).toHaveLength(1);
    expect(markers[0]).toHaveTextContent('settings.context.excludedInStack');
    expect(markers[0]).toHaveAttribute('title', 'settings.context.excludedInStackHint');
    const row = screen.getByText('C:\\Users\\me\\Documents\\kairos-mvp').parentElement as HTMLElement;
    expect(within(row).getByTestId('scan-dir-excluded')).toBeInTheDocument();
    // The not-on-this-machine marker is unchanged and separate.
    expect(screen.getByText('settings.context.notOnThisMachine')).toBeInTheDocument();
  });

  it('marks nothing when no root is excluded', () => {
    renderWith([]);
    expect(screen.queryByTestId('scan-dir-excluded')).toBeNull();
  });
});
