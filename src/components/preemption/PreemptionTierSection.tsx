// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { ItemCard } from './PreemptionCard';

interface PreemptionTierSectionProps {
  dotColor: string;
  borderColor: string;
  title: string;
  subtitle: string;
  items: EvidenceItem[];
  surfacedRef: React.RefObject<Set<string>>;
  onDismiss: (id: string) => void;
  emptyText: string;
}

export const PreemptionTierSection = memo(function PreemptionTierSection({
  dotColor,
  borderColor,
  title,
  subtitle,
  items,
  surfacedRef,
  onDismiss,
  emptyText,
}: PreemptionTierSectionProps) {
  return (
    <section className="mb-4" aria-label={title}>
      <div className="bg-bg-secondary rounded-lg border overflow-hidden" style={{ borderColor }}>
        <div className="px-4 py-3 border-b border-border flex items-center gap-2">
          <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: dotColor }} />
          <h3 className="text-sm font-medium text-white flex-1">{title}</h3>
          <span className="text-xs text-[#8A8A8A]">{subtitle}</span>
        </div>
        {items.length > 0 ? (
          <div className="p-4 space-y-4">
            {items.map(item => (
              <ItemCard key={item.id} item={item} surfacedRef={surfacedRef} onDismiss={onDismiss} />
            ))}
          </div>
        ) : (
          <div className="px-4 py-4">
            <p className="text-xs text-[#8A8A8A]">{emptyText}</p>
          </div>
        )}
      </div>
    </section>
  );
});
