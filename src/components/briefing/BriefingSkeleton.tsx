// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { SKELETON_WIDTHS } from './BriefingHelpers';

/**
 * Loading skeleton displayed while the AI briefing is being generated.
 * Extracted from BriefingView to keep the main component under 350 lines.
 */
export const BriefingSkeleton = memo(function BriefingSkeleton() {
  return (
    <div className="bg-bg-primary rounded-lg" role="status" aria-busy="true" aria-label="Loading briefing">
      <div className="space-y-4">
        {/* Skeleton header */}
        <div className="bg-bg-secondary rounded-lg border border-border p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
              <div className="w-4 h-4 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
            </div>
            <div>
              <div className="h-5 w-48 bg-bg-tertiary rounded animate-pulse" />
              <div className="h-3 w-32 bg-bg-tertiary rounded animate-pulse mt-2" />
            </div>
          </div>
          {/* Skeleton lines */}
          <div className="space-y-3">
            {SKELETON_WIDTHS.map((w, i) => (
              <div key={i} className="h-4 bg-bg-tertiary rounded animate-pulse" style={{ width: `${w}%` }} />
            ))}
          </div>
        </div>
        {/* Skeleton cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="bg-bg-secondary rounded-lg border border-border p-4">
              <div className="flex gap-3">
                <div className="w-10 h-6 bg-bg-tertiary rounded animate-pulse" />
                <div className="flex-1 space-y-2">
                  <div className="h-4 bg-bg-tertiary rounded animate-pulse" />
                  <div className="h-3 bg-bg-tertiary rounded animate-pulse w-3/4" />
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
});
