// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useAppStore } from '../store';

export const EmbeddingStatusIndicator = memo(function EmbeddingStatusIndicator() {
  const status = useAppStore(s => s.embeddingStatus);

  if (!status || status === 'active') return null;

  return (
    <div className="mb-4 px-4 py-3 bg-amber-500/10 border border-amber-500/20 rounded-lg flex items-center gap-3">
      <div className="w-2 h-2 rounded-full bg-amber-400 flex-shrink-0" />
      <div>
        <p className="text-xs font-medium text-amber-400">Semantic scoring limited</p>
        <p className="text-xs text-text-muted">
          {status === 'degraded'
            ? 'Embeddings using fallback. Install Ollama or add an API key for better results.'
            : 'Embedding service unavailable. Scoring uses keyword and dependency signals only.'}
        </p>
      </div>
    </div>
  );
});
