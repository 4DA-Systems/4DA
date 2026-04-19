// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useMemo } from 'react';
import { cmd } from '../../lib/commands';
import { useTranslation } from 'react-i18next';

// ============================================================================
// Types
// ============================================================================

interface TranslationEntry {
  english: string;
  translated: string | null;
  status: 'overridden' | 'translated' | 'untranslated';
}

interface TranslationStatus {
  language: string;
  total_keys: number;
  translated_keys: number;
  percentage: number;
}

type Namespace = 'all' | 'ui' | 'coach' | 'streets' | 'errors';
const NAMESPACES: Namespace[] = ['all', 'ui', 'coach', 'streets', 'errors'];

interface TranslationEditorProps { language: string }

// ============================================================================
// Design tokens & computed styles
// ============================================================================

const BG = { primary: '#0A0A0A', secondary: '#141414', tertiary: '#1F1F1F' };
const TEXT = { primary: '#FFFFFF', secondary: '#A0A0A0', muted: '#8A8A8A' };
const ACCENT = { gold: '#D4AF37', success: '#22C55E', error: '#EF4444' };
const BORDER = '#2A2A2A';
const NS_COLORS: Record<string, string> = {
  ui: '#3B82F6', coach: ACCENT.gold, streets: ACCENT.success, errors: ACCENT.error,
};
const STATUS_COLORS: Record<string, string> = {
  overridden: ACCENT.gold, translated: ACCENT.success, untranslated: TEXT.muted,
};

const s = {
  container: { background: BG.tertiary, borderRadius: 8, padding: 20, border: `1px solid ${BORDER}` },
  header: { display: 'flex' as const, alignItems: 'center' as const, gap: 12, marginBottom: 16 },
  icon: {
    width: 32, height: 32, background: 'rgba(212,175,55,0.2)', borderRadius: 8,
    display: 'flex' as const, alignItems: 'center' as const, justifyContent: 'center' as const,
    flexShrink: 0, color: ACCENT.gold, fontSize: 14,
  },
  title: { color: TEXT.primary, fontWeight: 500, fontSize: 14, margin: 0 },
  desc: { color: TEXT.muted, fontSize: 12, marginTop: 4 },
  bar: { flex: 1, height: 6, background: BG.primary, borderRadius: 3, overflow: 'hidden' as const },
  barLabel: { color: TEXT.secondary, fontSize: 11, whiteSpace: 'nowrap' as const, minWidth: 80 },
  controls: { display: 'flex' as const, gap: 8, marginBottom: 12, flexWrap: 'wrap' as const },
  search: {
    flex: 1, minWidth: 180, padding: '6px 10px', background: BG.primary,
    border: `1px solid ${BORDER}`, borderRadius: 6, color: TEXT.primary, fontSize: 12, outline: 'none',
  },
  list: {
    maxHeight: 360, overflowY: 'auto' as const,
    border: `1px solid ${BORDER}`, borderRadius: 6, background: BG.primary,
  },
  row: {
    display: 'flex' as const, alignItems: 'center' as const, gap: 8,
    padding: '8px 12px', borderBottom: `1px solid ${BG.tertiary}`, fontSize: 12,
  },
  keyName: {
    color: TEXT.secondary, fontFamily: 'JetBrains Mono, monospace',
    fontSize: 11, minWidth: 140, flexShrink: 0,
  },
  val: { color: TEXT.primary, flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' as const },
  valUntranslated: { color: TEXT.muted, fontStyle: 'italic', flex: 1 },
  valOverride: { color: ACCENT.gold, flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' as const },
  btn: {
    padding: '2px 6px', background: 'transparent', borderWidth: 1,
    borderStyle: 'solid' as const, borderColor: BORDER, borderRadius: 4,
    color: TEXT.secondary, cursor: 'pointer', fontSize: 11, flexShrink: 0,
  },
  editInput: {
    flex: 1, padding: '4px 8px', background: BG.secondary,
    border: `1px solid ${ACCENT.gold}`, borderRadius: 4, color: TEXT.primary, fontSize: 12, outline: 'none',
  },
  noResults: { padding: 24, textAlign: 'center' as const, color: TEXT.muted, fontSize: 12 },
  auto: {
    padding: '6px 12px', background: ACCENT.gold, color: '#000', border: 'none',
    borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: 'pointer',
  },
};

function nsBtn(active: boolean) {
  return {
    padding: '4px 10px', borderRadius: 4, fontSize: 11, cursor: 'pointer',
    border: `1px solid ${active ? ACCENT.gold : BORDER}`,
    background: active ? 'rgba(212,175,55,0.15)' : BG.secondary,
    color: active ? ACCENT.gold : TEXT.secondary,
    fontWeight: active ? 600 : 400,
  };
}

function nsBadge(ns: string) {
  const c = NS_COLORS[ns] || TEXT.muted;
  return {
    padding: '1px 6px', borderRadius: 3, background: `${c}22`, color: c,
    fontSize: 10, fontWeight: 600, textTransform: 'uppercase' as const,
    flexShrink: 0, minWidth: 40, textAlign: 'center' as const,
  };
}

function statusDot(status: string) {
  return {
    width: 6, height: 6, borderRadius: '50%',
    background: STATUS_COLORS[status] || TEXT.muted, flexShrink: 0,
  };
}

function barFill(pct: number) {
  return {
    width: `${pct}%`, height: '100%', borderRadius: 3,
    background: pct === 100 ? ACCENT.success : ACCENT.gold, transition: 'width 0.3s ease',
  };
}

// ============================================================================
// Component
// ============================================================================

export function TranslationEditor({ language }: TranslationEditorProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<TranslationStatus | null>(null);
  const [entries, setEntries] = useState<Record<string, TranslationEntry>>({});
  const [search, setSearch] = useState('');
  const [nsFilter, setNsFilter] = useState<Namespace>('all');
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [editValue, setEditValue] = useState('');
  const [translating, setTranslating] = useState(false);

  const loadData = useCallback(async () => {
    if (!language || language === 'en') return;
    try {
      const [st, en] = await Promise.all([
        cmd('get_translation_status', { lang: language }),
        cmd('get_all_translations', { lang: language }),
      ]);
      setStatus(st as unknown as TranslationStatus);
      setEntries(en as unknown as Record<string, TranslationEntry>);
    } catch { /* non-critical */ }
  }, [language]);

  useEffect(() => { loadData(); }, [loadData]);

  const filteredEntries = useMemo(() => {
    return Object.entries(entries)
      .filter(([key, entry]) => {
        if (nsFilter !== 'all' && key.split(':')[0] !== nsFilter) return false;
        if (search) {
          const q = search.toLowerCase();
          const shortKey = key.split(':')[1] || key;
          return shortKey.toLowerCase().includes(q)
            || entry.english.toLowerCase().includes(q)
            || (entry.translated?.toLowerCase().includes(q) ?? false);
        }
        return true;
      })
      .sort((a, b) => {
        const ord: Record<string, number> = { untranslated: 0, overridden: 1, translated: 2 };
        const diff = (ord[a[1].status] ?? 3) - (ord[b[1].status] ?? 3);
        return diff !== 0 ? diff : a[0].localeCompare(b[0]);
      });
  }, [entries, nsFilter, search]);

  const handleEdit = useCallback((key: string, val: string) => {
    setEditingKey(key);
    setEditValue(val);
  }, []);

  const handleSave = useCallback(async () => {
    if (!editingKey) return;
    const [namespace, ...rest] = editingKey.split(':') as [string, ...string[]];
    try {
      await cmd('save_translation_override', {
        lang: language, namespace, key: rest.join(':'), value: editValue,
      });
      setEditingKey(null);
      await loadData();
    } catch { /* silent */ }
  }, [editingKey, editValue, language, loadData]);

  const handleCancel = useCallback(() => { setEditingKey(null); setEditValue(''); }, []);

  const handleAutoTranslate = useCallback(async () => {
    setTranslating(true);
    try { await cmd('trigger_translation', { lang: language }); await loadData(); }
    catch { /* silent */ }
    setTranslating(false);
  }, [language, loadData]);

  if (!language || language === 'en') return null;
  const pct = status?.percentage ?? 0;

  return (
    <div style={s.container}>
      {/* Header */}
      <div style={s.header}>
        <div style={s.icon}>Aa</div>
        <div>
          <h3 style={s.title}>{t('settings.translations.title')}</h3>
          <p style={s.desc}>{t('settings.translations.description')}</p>
        </div>
      </div>

      {/* Completeness */}
      {status && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
          <div style={s.bar}><div style={barFill(pct)} /></div>
          <span style={s.barLabel}>
            {t('settings.translations.completeness', {
              percent: Math.round(pct), translated: status.translated_keys, total: status.total_keys,
            })}
          </span>
        </div>
      )}

      {/* Controls */}
      <div style={s.controls}>
        <input type="text" value={search} onChange={(e) => setSearch(e.target.value)}
          placeholder={t('settings.translations.search')} style={s.search} />
        {NAMESPACES.map((ns) => (
          <button key={ns}
            aria-label={ns === 'all' ? t('settings.translations.allNamespaces') : ns}
            onClick={() => setNsFilter(ns)} style={nsBtn(nsFilter === ns)}>
            {ns === 'all' ? t('settings.translations.allNamespaces') : ns}
          </button>
        ))}
        <button onClick={handleAutoTranslate} disabled={translating || pct === 100}
          aria-label={translating ? t('settings.translations.translating') : t('settings.translations.autoTranslate')}
          style={{ ...s.auto, ...(translating || pct === 100 ? { opacity: 0.5, cursor: 'not-allowed' } : {}) }}>
          {translating ? t('settings.translations.translating') : t('settings.translations.autoTranslate')}
        </button>
      </div>

      {/* Translation rows */}
      <div style={s.list}>
        {filteredEntries.length === 0 ? (
          <div style={s.noResults}>{t('settings.translations.noResults')}</div>
        ) : filteredEntries.map(([fullKey, entry]) => {
          const [ns, ...keyParts] = fullKey.split(':') as [string, ...string[]];
          const shortKey = keyParts.join(':');
          const isEditing = editingKey === fullKey;
          const displayValue = entry.translated ?? entry.english;

          return (
            <div key={fullKey} style={s.row}>
              <div style={statusDot(entry.status)} title={entry.status} />
              <span style={nsBadge(ns)}>{ns}</span>
              <span style={s.keyName}>{shortKey}</span>
              {isEditing ? (
                <>
                  {/* eslint-disable-next-line jsx-a11y/no-autofocus -- intentional: focus edit input when user clicks to edit a translation */}
                  <input type="text" value={editValue} onChange={(e) => setEditValue(e.target.value)} style={s.editInput} autoFocus onKeyDown={(e) => { if (e.key === 'Enter') handleSave(); if (e.key === 'Escape') handleCancel(); }} />
                  <button onClick={handleSave} aria-label={t('settings.translations.save')}
                    style={{ ...s.btn, borderColor: ACCENT.success, color: ACCENT.success }}>
                    {t('action.save')}
                  </button>
                  <button onClick={handleCancel} aria-label={t('settings.translations.cancelEdit')} style={s.btn}>
                    {t('action.cancel')}
                  </button>
                </>
              ) : (
                <>
                  <span style={entry.status === 'untranslated' ? s.valUntranslated
                    : entry.status === 'overridden' ? s.valOverride : s.val}
                    title={`EN: ${entry.english}`}>
                    {entry.status === 'untranslated' ? entry.english : displayValue}
                  </span>
                  <button onClick={() => handleEdit(fullKey, displayValue)}
                    aria-label={t('settings.translations.edit')} style={s.btn}>
                    {t('settings.translations.edit')}
                  </button>
                </>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
