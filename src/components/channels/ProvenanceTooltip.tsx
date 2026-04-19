// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { RenderProvenance } from '../../types/channels';

interface Props {
  provenance: RenderProvenance;
  children: React.ReactNode;
}

export function ProvenanceTooltip({ provenance, children }: Props) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Close on click outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as globalThis.Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  // Close on Escape key
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [open]);

  return (
    <span className="relative inline-block" ref={ref}>
      <button
        onClick={() => setOpen(!open)}
        className="bg-cyan-500/20 text-cyan-400 text-xs rounded px-1 cursor-pointer hover:bg-cyan-500/30 transition-colors font-mono"
        aria-expanded={open}
        aria-haspopup="true"
      >
        {children}
      </button>
      {open && (
        <div
          className="absolute z-50 bottom-full start-0 mb-2 w-72 bg-bg-primary border border-border rounded-lg shadow-xl p-3 text-sm"
          role="tooltip"
        >
          <p className="text-text-muted text-xs mb-2 font-medium uppercase tracking-wide">
            {t('channels.provenance')}
          </p>
          {provenance.source_titles.map((title, i) => (
            <div key={i} className="mb-2 last:mb-0">
              <p className="text-white text-sm font-medium leading-tight">
                {title}
              </p>
              {provenance.source_urls[i] && (
                <a
                  href={provenance.source_urls[i]}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-cyan-400 text-xs hover:underline truncate block mt-0.5"
                >
                  {provenance.source_urls[i]}
                </a>
              )}
            </div>
          ))}
        </div>
      )}
    </span>
  );
}
