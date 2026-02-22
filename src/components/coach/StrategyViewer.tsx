import { useCallback, useState } from 'react';
import { useAppStore } from '../../store';

// ---------------------------------------------------------------------------
// Markdown Prose Renderer (lightweight)
// ---------------------------------------------------------------------------

function MarkdownProse({ content }: { content: string }) {
  // Split into lines and render with basic heading/list/paragraph styling.
  // This avoids pulling in a full markdown library for the strategy doc.
  const lines = content.split('\n');
  const elements: React.ReactNode[] = [];
  let listBuffer: string[] = [];
  let olBuffer: string[] = [];

  const flushList = (key: string) => {
    if (listBuffer.length > 0) {
      elements.push(
        <ul key={`ul-${key}`} className="list-disc list-inside space-y-1 text-xs text-[#A0A0A0] leading-relaxed pl-2">
          {listBuffer.map((item, j) => (
            <li key={j}>{item}</li>
          ))}
        </ul>,
      );
      listBuffer = [];
    }
    if (olBuffer.length > 0) {
      elements.push(
        <ol key={`ol-${key}`} className="list-decimal list-inside space-y-1 text-xs text-[#A0A0A0] leading-relaxed pl-2">
          {olBuffer.map((item, j) => (
            <li key={j}>{item}</li>
          ))}
        </ol>,
      );
      olBuffer = [];
    }
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    // Headings
    if (trimmed.startsWith('### ')) {
      flushList(String(i));
      elements.push(
        <h5 key={i} className="text-xs font-semibold text-white mt-4 mb-1">
          {trimmed.slice(4)}
        </h5>,
      );
    } else if (trimmed.startsWith('## ')) {
      flushList(String(i));
      elements.push(
        <h4 key={i} className="text-sm font-semibold text-white mt-5 mb-1.5">
          {trimmed.slice(3)}
        </h4>,
      );
    } else if (trimmed.startsWith('# ')) {
      flushList(String(i));
      elements.push(
        <h3 key={i} className="text-base font-bold text-white mt-6 mb-2">
          {trimmed.slice(2)}
        </h3>,
      );
    } else if (/^[-*] /.test(trimmed)) {
      // Unordered list item
      listBuffer.push(trimmed.slice(2));
    } else if (/^\d+\.\s/.test(trimmed)) {
      // Ordered list item
      olBuffer.push(trimmed.replace(/^\d+\.\s/, ''));
    } else if (trimmed === '') {
      flushList(String(i));
    } else {
      flushList(String(i));
      elements.push(
        <p key={i} className="text-xs text-[#A0A0A0] leading-relaxed">
          {trimmed}
        </p>,
      );
    }
  }
  flushList('end');

  return <div className="space-y-2">{elements}</div>;
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function StrategyViewer() {
  const doc = useAppStore((s) => s.strategyDocument);
  const loading = useAppStore((s) => s.coachLoading);
  const generateStrategy = useAppStore((s) => s.generateStrategy);
  const [copied, setCopied] = useState(false);

  const handleGenerate = useCallback(() => {
    generateStrategy();
  }, [generateStrategy]);

  const handleCopy = useCallback(() => {
    if (!doc) return;
    navigator.clipboard.writeText(doc).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [doc]);

  return (
    <div className="space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold text-white">Strategy Document</h3>
          <p className="text-xs text-[#666] mt-0.5">
            Personalized revenue strategy based on your profile
          </p>
        </div>
        <div className="flex items-center gap-2">
          {doc && (
            <button
              onClick={handleCopy}
              className="px-3 py-2 text-xs font-medium text-[#A0A0A0] border border-[#2A2A2A] rounded-lg hover:bg-[#1F1F1F] hover:text-white transition-colors"
            >
              {copied ? 'Copied' : 'Export'}
            </button>
          )}
          <button
            onClick={handleGenerate}
            disabled={loading}
            className="px-4 py-2 text-sm font-medium bg-[#D4AF37] text-black rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {loading ? 'Generating...' : doc ? 'Regenerate' : 'Generate Strategy Document'}
          </button>
        </div>
      </div>

      {/* Loading State */}
      {loading && !doc && (
        <div className="flex items-center justify-center py-16">
          <div className="flex flex-col items-center gap-3">
            <div className="w-5 h-5 border-2 border-[#D4AF37] border-t-transparent rounded-full animate-spin" />
            <p className="text-xs text-[#A0A0A0]">
              Generating your personalized strategy document...
            </p>
            <p className="text-[10px] text-[#666]">
              This may take a moment while the AI analyzes your profile
            </p>
          </div>
        </div>
      )}

      {/* Document Display */}
      {doc && (
        <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl overflow-hidden">
          <div className="flex items-center justify-between px-5 py-3 border-b border-[#2A2A2A]">
            <span className="text-[10px] text-[#666] uppercase tracking-wide font-medium">
              Strategy Document
            </span>
            <span className="text-[10px] text-[#666]">
              {doc.split('\n').length} lines
            </span>
          </div>
          <div className="px-5 py-4 max-h-[600px] overflow-y-auto">
            <MarkdownProse content={doc} />
          </div>
        </div>
      )}

      {/* Empty State */}
      {!loading && !doc && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-12 h-12 bg-[#D4AF37]/10 rounded-xl flex items-center justify-center mb-3">
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="#D4AF37"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
              <line x1="16" y1="13" x2="8" y2="13" />
              <line x1="16" y1="17" x2="8" y2="17" />
              <polyline points="10 9 9 9 8 9" />
            </svg>
          </div>
          <p className="text-sm text-[#A0A0A0] max-w-sm">
            Generate a comprehensive strategy document tailored to your
            sovereign profile, recommended engines, and current progress.
          </p>
        </div>
      )}
    </div>
  );
}
