import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface ExportManifest {
  export_id: string;
  created_at: string;
  format: string;
  sections: string[];
  total_records: number;
}

const EXPORT_SECTION_KEYS = [
  { key: 'user_profile', i18nKey: 'enterprise.export.section.profile', icon: '\u{1F464}' },
  { key: 'decisions', i18nKey: 'enterprise.export.section.decisions', icon: '\u{1F4DD}' },
  { key: 'signals', i18nKey: 'enterprise.export.section.signals', icon: '\u{1F4E1}' },
  { key: 'sources', i18nKey: 'enterprise.export.section.sources', icon: '\u{1F517}' },
  { key: 'briefings', i18nKey: 'enterprise.export.section.briefings', icon: '\u{1F4C4}' },
  { key: 'feedback', i18nKey: 'enterprise.export.section.feedback', icon: '\u{1F44D}' },
  { key: 'learned_behavior', i18nKey: 'enterprise.export.section.learned', icon: '\u{1F9E0}' },
];

export function DataExportPanel() {
  const { t } = useTranslation();

  const [exports, setExports] = useState<ExportManifest[]>([]);
  const [, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [exportResult, setExportResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [selectedSections, setSelectedSections] = useState<Set<string>>(
    new Set(EXPORT_SECTION_KEYS.map(s => s.key))
  );
  const [singleExporting, setSingleExporting] = useState<string | null>(null);

  useEffect(() => {
    loadExports();
  }, []);

  const loadExports = async () => {
    setLoading(true);
    try {
      const list = await cmd('list_exports');
      setExports(list as unknown as ExportManifest[]);
    } catch { /* silent */ }
    setLoading(false);
  };

  const handleExportAll = async () => {
    setExporting(true);
    setExportResult(null);
    try {
      const manifest = await cmd('export_all_data', { format: 'json' });
      const m = manifest as unknown as ExportManifest;
      setExportResult({ ok: true, msg: `Exported ${m.total_records} records to ${m.export_id}.json` });
      loadExports();
    } catch {
      setExportResult({ ok: false, msg: 'Export failed. Check logs for details.' });
    }
    setExporting(false);
    setTimeout(() => setExportResult(null), 5000);
  };

  const handleExportSection = async (section: string) => {
    setSingleExporting(section);
    try {
      const data = await cmd('export_section', { section, format: 'json' });
      // Create a download link
      const blob = new Blob([data as unknown as string], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `4da-${section}-export.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch { /* silent */ }
    setSingleExporting(null);
  };

  const handleDeleteExport = async (exportId: string) => {
    try {
      await cmd('delete_export', { exportId });
      setExports(prev => prev.filter(e => e.export_id !== exportId));
    } catch { /* silent */ }
  };

  const toggleSection = (key: string) => {
    setSelectedSections(prev => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-white">
            {t('enterprise.export.title')}
          </h3>
          <p className="text-[10px] text-text-muted mt-0.5">
            {t('enterprise.export.description')}
          </p>
        </div>
      </div>

      {/* Status */}
      {exportResult && (
        <div className={`px-3 py-2 rounded text-xs ${
          exportResult.ok ? 'bg-success/10 text-success' : 'bg-error/10 text-error'
        }`}>
          {exportResult.msg}
        </div>
      )}

      {/* Section Selection */}
      <div>
        <h4 className="text-xs font-medium text-text-secondary mb-2">
          {t('enterprise.export.sections')}
        </h4>
        <div className="grid grid-cols-2 gap-1.5">
          {EXPORT_SECTION_KEYS.map(section => {
            const label = t(section.i18nKey);
            return (
              <div
                key={section.key}
                className="flex items-center justify-between px-3 py-2 bg-bg-primary rounded-lg border border-border/50"
              >
                <label className="flex items-center gap-2 cursor-pointer flex-1">
                  <input
                    type="checkbox"
                    checked={selectedSections.has(section.key)}
                    onChange={() => toggleSection(section.key)}
                    className="rounded border-border"
                  />
                  <span className="text-xs text-white">{label}</span>
                </label>
                <button
                  onClick={() => handleExportSection(section.key)}
                  disabled={singleExporting === section.key}
                  className="text-[10px] text-text-muted hover:text-success transition-colors ms-2"
                  title={t('enterprise.export.exportOnly', { label })}
                >
                  {singleExporting === section.key ? '...' : '\u{2B07}'}
                </button>
              </div>
            );
          })}
        </div>
      </div>

      {/* Export All Button */}
      <button
        onClick={handleExportAll}
        disabled={exporting || selectedSections.size === 0}
        className="w-full px-4 py-2.5 text-xs bg-success/15 text-success rounded-lg hover:bg-success/25 transition-colors disabled:opacity-50 font-medium"
      >
        {exporting
          ? t('enterprise.export.exporting')
          : t('enterprise.export.exportAll', { selectedSections: selectedSections.size })
        }
      </button>

      {/* Previous Exports */}
      {exports.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-secondary mb-2">
            {t('enterprise.export.previous')}
          </h4>
          <div className="space-y-1.5">
            {exports.map(exp => (
              <div
                key={exp.export_id}
                className="flex items-center justify-between px-3 py-2 bg-bg-primary rounded-lg border border-border/50"
              >
                <div>
                  <span className="text-xs text-white font-mono">{exp.export_id.slice(0, 12)}...</span>
                  <span className="text-[10px] text-text-muted ms-2">
                    {exp.format.toUpperCase()} &middot; {exp.total_records} records &middot; {exp.sections.length} sections
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-[10px] text-text-muted">{formatDate(exp.created_at)}</span>
                  <button
                    onClick={() => handleDeleteExport(exp.export_id)}
                    className="text-[10px] text-text-muted hover:text-error transition-colors"
                    aria-label="Delete export"
                  >
                    &#10005;
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Privacy Note */}
      <div className="px-3 py-2 rounded-lg bg-bg-primary border border-border/50">
        <p className="text-[9px] text-text-muted leading-relaxed">
          {t('enterprise.export.privacy')}
        </p>
      </div>
    </div>
  );
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
  } catch {
    return iso;
  }
}
