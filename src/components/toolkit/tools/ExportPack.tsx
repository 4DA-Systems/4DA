import { useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

interface ExportPackResult {
  markdown: string;
  has_dna: boolean;
  has_radar: boolean;
  has_decisions: boolean;
}

function SectionToggle({
  label,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label
      className={`flex items-center gap-2 px-3 py-2 text-sm rounded-lg border transition-all cursor-pointer select-none ${
        disabled
          ? 'bg-[#1F1F1F] border-[#2A2A2A] text-[#666] cursor-not-allowed'
          : checked
            ? 'bg-[#141414] border-white/20 text-white'
            : 'bg-[#141414] border-[#2A2A2A] text-[#A0A0A0] hover:border-white/10'
      }`}
    >
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="accent-white w-3.5 h-3.5"
      />
      {label}
    </label>
  );
}

function formatDate(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export default function ExportPack() {
  const { t } = useTranslation();
  const [result, setResult] = useState<ExportPackResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Section toggles
  const [showDna, setShowDna] = useState(true);
  const [showRadar, setShowRadar] = useState(true);
  const [showDecisions, setShowDecisions] = useState(true);

  const generate = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await invoke<ExportPackResult>('toolkit_generate_export_pack');
      setResult(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setResult(null);
    } finally {
      setLoading(false);
    }
  }, []);

  // Filter markdown by sections
  const filteredMarkdown = useMemo(() => {
    if (!result) return '';
    const sections = result.markdown.split(/(?=^## )/m);
    return sections
      .filter((section) => {
        const lower = section.toLowerCase();
        if (lower.startsWith('## dna') || lower.startsWith('## developer dna') || lower.startsWith('## stack dna')) {
          return showDna;
        }
        if (lower.startsWith('## radar') || lower.startsWith('## tech radar') || lower.startsWith('## technology radar')) {
          return showRadar;
        }
        if (lower.startsWith('## decision') || lower.startsWith('## architectural decision')) {
          return showDecisions;
        }
        // Keep preamble and other sections
        return true;
      })
      .join('')
      .trim();
  }, [result, showDna, showRadar, showDecisions]);

  const copyMarkdown = useCallback(async () => {
    if (!filteredMarkdown) return;
    try {
      await navigator.clipboard.writeText(filteredMarkdown);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setError('Failed to copy to clipboard');
    }
  }, [filteredMarkdown]);

  const downloadMarkdown = useCallback(() => {
    if (!filteredMarkdown) return;
    const blob = new Blob([filteredMarkdown], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `4da-profile-${formatDate()}.md`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, [filteredMarkdown]);

  return (
    <div className="space-y-4">
      {/* Controls */}
      <div className="flex items-center gap-3 flex-wrap">
        <button
          onClick={generate}
          disabled={loading}
          className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {loading ? (
            <>
              <div className="w-3.5 h-3.5 border-2 border-[#0A0A0A]/30 border-t-[#0A0A0A] rounded-full animate-spin" />
              {t('toolkit.exportPack.generating')}
            </>
          ) : (
            <>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 8v13H3V8" /><path d="M1 3h22v5H1z" /><path d="M10 12h4" />
              </svg>
              {t('toolkit.exportPack.generateExport')}
            </>
          )}
        </button>

        {/* Section toggles */}
        <div className="flex items-center gap-2 ml-auto">
          <span className="text-xs text-[#666]">{t('toolkit.exportPack.sections')}:</span>
          <SectionToggle
            label="DNA"
            checked={showDna}
            disabled={result ? !result.has_dna : false}
            onChange={setShowDna}
          />
          <SectionToggle
            label="Radar"
            checked={showRadar}
            disabled={result ? !result.has_radar : false}
            onChange={setShowRadar}
          />
          <SectionToggle
            label="Decisions"
            checked={showDecisions}
            disabled={result ? !result.has_decisions : false}
            onChange={setShowDecisions}
          />
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center gap-3 px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#EF4444" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <span className="text-sm text-[#EF4444] flex-1">{error}</span>
          <button onClick={() => setError(null)} className="text-[#EF4444]/60 hover:text-[#EF4444] text-xs">
            {t('action.dismiss')}
          </button>
        </div>
      )}

      {/* Preview */}
      {result && filteredMarkdown ? (
        <div className="space-y-3">
          <div className="bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg p-4 max-h-[500px] overflow-auto">
            <pre className="text-sm font-mono text-[#A0A0A0] whitespace-pre-wrap leading-relaxed">
              {filteredMarkdown}
            </pre>
          </div>

          {/* Action buttons */}
          <div className="flex items-center gap-3">
            <button
              onClick={copyMarkdown}
              className="flex items-center gap-2 px-3 py-2 text-xs bg-[#141414] border border-[#2A2A2A] rounded-lg hover:text-white hover:border-white/20 transition-all"
            >
              {copied ? (
                <>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#22C55E" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                  <span className="text-[#22C55E]">{t('action.copied')}</span>
                </>
              ) : (
                <>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
                  </svg>
                  <span className="text-[#A0A0A0]">{t('toolkit.exportPack.copyMarkdown')}</span>
                </>
              )}
            </button>

            <button
              onClick={downloadMarkdown}
              className="flex items-center gap-2 px-3 py-2 text-xs text-[#A0A0A0] bg-[#141414] border border-[#2A2A2A] rounded-lg hover:text-white hover:border-white/20 transition-all"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              {t('toolkit.exportPack.downloadMd')}
            </button>

            <span className="text-[10px] text-[#666] ml-auto font-mono">
              {filteredMarkdown.length.toLocaleString()} chars
            </span>
          </div>
        </div>
      ) : !loading && !error ? (
        <div className="flex flex-col items-center justify-center py-14 text-center">
          <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="#666" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mb-3">
            <path d="M21 8v13H3V8" />
            <path d="M1 3h22v5H1z" />
            <path d="M10 12h4" />
          </svg>
          <p className="text-sm text-[#A0A0A0] mb-1">{t('toolkit.exportPack.empty')}</p>
          <p className="text-xs text-[#666]">
            {t('toolkit.exportPack.emptyHint')}
          </p>
        </div>
      ) : null}
    </div>
  );
}
