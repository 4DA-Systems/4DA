import { useEffect, useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

// Inline template type (previously in types/coach.ts)
interface Template {
  id: string;
  title: string;
  description: string;
  category: string;
  content: string;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function CategoryFilter({
  categories,
  active,
  onSelect,
}: {
  categories: string[];
  active: string;
  onSelect: (cat: string) => void;
}) {
  return (
    <div className="flex flex-wrap gap-2">
      <button
        onClick={() => onSelect('all')}
        aria-label="Filter: All categories"
        className={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
          active === 'all'
            ? 'bg-[#D4AF37]/15 text-[#D4AF37] border-[#D4AF37]/30'
            : 'bg-bg-tertiary text-text-secondary border-border hover:text-white hover:border-[#D4AF37]/20'
        }`}
      >
        All
      </button>
      {categories.map(cat => (
        <button
          key={cat}
          onClick={() => onSelect(cat)}
          aria-label={`Filter: ${cat}`}
          className={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
            active === cat
              ? 'bg-[#D4AF37]/15 text-[#D4AF37] border-[#D4AF37]/30'
              : 'bg-bg-tertiary text-text-secondary border-border hover:text-white hover:border-[#D4AF37]/20'
          }`}
        >
          {cat}
        </button>
      ))}
    </div>
  );
}

function CategoryBadge({ category }: { category: string }) {
  return (
    <span className="px-1.5 py-0.5 text-[10px] font-medium rounded bg-[#D4AF37]/10 text-[#D4AF37] border border-[#D4AF37]/20">
      {category}
    </span>
  );
}

function TemplateCard({
  template,
  onOpen,
}: {
  template: Template;
  onOpen: (t: Template) => void;
}) {
  return (
    <button
      onClick={() => onOpen(template)}
      aria-label={`Open template: ${template.title}`}
      className="w-full text-left bg-bg-secondary border border-border rounded-xl p-4 transition-colors hover:border-[#D4AF37]/30 group"
    >
      <div className="flex items-start justify-between gap-2 mb-2">
        <h4 className="text-sm font-semibold text-white group-hover:text-[#D4AF37] transition-colors leading-snug">
          {template.title}
        </h4>
        <CategoryBadge category={template.category} />
      </div>
      <p className="text-xs text-text-secondary leading-relaxed line-clamp-2">
        {template.description}
      </p>
    </button>
  );
}

function TemplateViewer({
  template,
  onClose,
}: {
  template: Template;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(template.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard API may not be available in all contexts
    }
  }, [template.content]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-primary/80 backdrop-blur-sm">
      <div className="bg-bg-secondary border border-border rounded-xl shadow-2xl w-full max-w-2xl max-h-[80vh] flex flex-col mx-4">
        {/* Viewer header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-border">
          <div className="flex items-center gap-3 min-w-0">
            <h3 className="text-sm font-semibold text-white truncate">
              {template.title}
            </h3>
            <CategoryBadge category={template.category} />
          </div>
          <div className="flex items-center gap-2 flex-shrink-0">
            <button
              onClick={handleCopy}
              aria-label={copied ? t('action.copied') : t('action.copy')}
              className={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
                copied
                  ? 'bg-[#22C55E]/15 text-[#22C55E] border-[#22C55E]/30'
                  : 'bg-bg-tertiary text-text-secondary border-border hover:text-white hover:border-[#D4AF37]/30'
              }`}
            >
              {copied ? t('action.copied') : t('action.copy')}
            </button>
            <button
              onClick={onClose}
              className="w-7 h-7 flex items-center justify-center rounded-lg text-[#666] hover:text-white hover:bg-bg-tertiary transition-colors"
              aria-label={t('action.close')}
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          <pre className="text-xs text-text-secondary leading-relaxed whitespace-pre-wrap font-[JetBrains_Mono,monospace]">
            {template.content}
          </pre>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function TemplateLibrary() {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [activeCategory, setActiveCategory] = useState('all');
  const [viewingTemplate, setViewingTemplate] = useState<Template | null>(null);

  useEffect(() => {
    invoke<Template[]>('get_templates')
      .then(setTemplates)
      .catch(() => { /* non-fatal */ });
  }, []);

  const categories = useMemo(() => {
    const cats = new Set(templates.map(t => t.category));
    return Array.from(cats).sort();
  }, [templates]);

  const filtered = useMemo(() => {
    if (activeCategory === 'all') return templates;
    return templates.filter(t => t.category === activeCategory);
  }, [templates, activeCategory]);

  return (
    <div className="space-y-5">
      {/* Header */}
      <div>
        <h3 className="text-sm font-semibold text-white">Templates</h3>
        <p className="text-xs text-[#666] mt-0.5">
          Actionable templates for launching and growing your revenue engines
        </p>
      </div>

      {/* Category filter */}
      {categories.length > 0 && (
        <CategoryFilter
          categories={categories}
          active={activeCategory}
          onSelect={setActiveCategory}
        />
      )}

      {/* Template grid */}
      {filtered.length > 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {filtered.map(template => (
            <TemplateCard
              key={template.id}
              template={template}
              onOpen={setViewingTemplate}
            />
          ))}
        </div>
      ) : (
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
          <p className="text-sm text-text-secondary max-w-sm">
            {templates.length === 0
              ? 'No templates available yet.'
              : 'No templates match this category.'}
          </p>
        </div>
      )}

      {/* Viewer modal */}
      {viewingTemplate && (
        <TemplateViewer
          template={viewingTemplate}
          onClose={() => setViewingTemplate(null)}
        />
      )}
    </div>
  );
}
