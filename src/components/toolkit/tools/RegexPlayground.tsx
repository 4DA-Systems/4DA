import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { FLAGS, HIGHLIGHT_COLORS, buildHighlightedSegments } from './regex-utils';
import type { MatchResult, Flag } from './regex-utils';

export default function RegexPlayground() {
  const { t } = useTranslation();
  const [pattern, setPattern] = useState('');
  const [flags, setFlags] = useState<Set<Flag>>(new Set(['g']));
  const [testString, setTestString] = useState('');
  const [replaceMode, setReplaceMode] = useState(false);
  const [replacePattern, setReplacePattern] = useState('');

  const [matches, setMatches] = useState<MatchResult[]>([]);
  const [regexError, setRegexError] = useState<string | null>(null);
  const [replacedText, setReplacedText] = useState('');

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const flagString = useMemo(() => {
    const sorted: Flag[] = ['g', 'i', 'm', 's', 'u'];
    return sorted.filter(f => flags.has(f)).join('');
  }, [flags]);

  const executeRegex = useCallback(() => {
    if (!pattern) {
      setMatches([]);
      setRegexError(null);
      setReplacedText('');
      return;
    }

    try {
      const re = new RegExp(pattern, flagString);
      setRegexError(null);

      const results: MatchResult[] = [];

      if (flags.has('g')) {
        let m: RegExpExecArray | null;
        re.lastIndex = 0;
        while ((m = re.exec(testString)) !== null) {
          results.push({
            text: m[0],
            index: m.index,
            length: m[0].length,
            groups: m.groups ? { ...m.groups } : {},
            groupEntries: Array.from(m).slice(1),
          });
          if (m[0].length === 0) re.lastIndex++; // prevent infinite loop on zero-width
        }
      } else {
        const m = re.exec(testString);
        if (m) {
          results.push({
            text: m[0],
            index: m.index,
            length: m[0].length,
            groups: m.groups ? { ...m.groups } : {},
            groupEntries: Array.from(m).slice(1),
          });
        }
      }

      setMatches(results);

      if (replaceMode) {
        try {
          setReplacedText(testString.replace(re, replacePattern));
        } catch {
          setReplacedText('');
        }
      }
    } catch (err) {
      setRegexError(err instanceof Error ? err.message : 'Invalid pattern');
      setMatches([]);
      setReplacedText('');
    }
  }, [pattern, flagString, testString, replaceMode, replacePattern, flags]);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(executeRegex, 100);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [executeRegex]);

  const toggleFlag = (flag: Flag) => {
    setFlags(prev => {
      const next = new Set(prev);
      if (next.has(flag)) next.delete(flag);
      else next.add(flag);
      return next;
    });
  };

  const segments = useMemo(
    () => buildHighlightedSegments(testString, matches),
    [testString, matches],
  );

  const hasGroups = matches.some(m => m.groupEntries.length > 0);
  const hasNamedGroups = matches.some(m => Object.keys(m.groups).length > 0);

  const isValid = pattern.length > 0 && !regexError;
  const isInvalid = pattern.length > 0 && !!regexError;

  return (
    <div className="flex flex-col gap-4 h-full min-h-0 p-4">
      {/* Pattern row */}
      <div className="flex items-center gap-3">
        <div className="relative flex-1">
          <span className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted font-mono text-sm select-none">
            /
          </span>
          <input
            type="text"
            value={pattern}
            onChange={e => setPattern(e.target.value)}
            placeholder={t('toolkit.regex.patternPlaceholder')}
            spellCheck={false}
            className={`w-full font-mono text-sm bg-bg-tertiary text-text-primary border rounded px-7 py-2 outline-none transition-colors ${
              isInvalid
                ? 'border-error'
                : isValid
                  ? 'border-success'
                  : 'border-border'
            } focus:border-text-muted`}
          />
          <span className="absolute right-3 top-1/2 -translate-y-1/2 text-text-muted font-mono text-sm select-none">
            /{flagString}
          </span>
        </div>

        <div className="flex items-center gap-1">
          {FLAGS.map(({ flag, label, title }) => (
            <button
              key={flag}
              title={title}
              onClick={() => toggleFlag(flag)}
              className={`w-8 h-8 rounded font-mono text-sm font-medium transition-colors ${
                flags.has(flag)
                  ? 'bg-accent-gold/20 text-accent-gold border border-accent-gold/40'
                  : 'bg-bg-tertiary text-text-muted border border-border hover:text-text-secondary'
              }`}
            >
              {label}
            </button>
          ))}
        </div>

        <div
          className={`w-3 h-3 rounded-full flex-shrink-0 transition-colors ${
            isInvalid ? 'bg-error' : isValid ? 'bg-success' : 'bg-border'
          }`}
          title={isInvalid ? 'Invalid' : isValid ? 'Valid' : 'Empty'}
        />
      </div>

      {regexError && (
        <div className="text-error text-xs font-mono px-1">{regexError}</div>
      )}

      {/* Replace mode toggle */}
      <div className="flex items-center gap-3">
        <button
          onClick={() => setReplaceMode(prev => !prev)}
          className={`text-xs px-3 py-1.5 rounded border transition-colors ${
            replaceMode
              ? 'bg-accent-gold/20 text-accent-gold border-accent-gold/40'
              : 'bg-bg-tertiary text-text-muted border-border hover:text-text-secondary'
          }`}
        >
          {t('toolkit.regex.replaceMode')}
        </button>
        <span className="text-text-muted text-xs">
          {t('toolkit.regex.matchCount', { count: matches.length })}
        </span>
      </div>

      {/* Replace pattern input */}
      {replaceMode && (
        <input
          type="text"
          value={replacePattern}
          onChange={e => setReplacePattern(e.target.value)}
          placeholder={t('toolkit.regex.replacePlaceholder')}
          spellCheck={false}
          className="font-mono text-sm bg-bg-tertiary text-text-primary border border-border rounded px-3 py-2 outline-none focus:border-text-muted"
        />
      )}

      {/* Main content: test string + highlighted preview */}
      <div className="flex gap-4 min-h-0 flex-1">
        <div className="flex flex-col gap-2 flex-1 min-h-0">
          <label className="text-text-secondary text-xs uppercase tracking-wide font-medium">
            {t('toolkit.regex.testString')}
          </label>
          <textarea
            value={testString}
            onChange={e => setTestString(e.target.value)}
            placeholder={t('toolkit.regex.testPlaceholder')}
            spellCheck={false}
            className="flex-1 font-mono text-sm bg-bg-tertiary text-text-primary border border-border rounded p-3 outline-none resize-none focus:border-text-muted min-h-[120px]"
          />
        </div>

        <div className="flex flex-col gap-2 flex-1 min-h-0">
          <label className="text-text-secondary text-xs uppercase tracking-wide font-medium">
            {replaceMode ? t('toolkit.regex.replacePreview') : t('toolkit.regex.matchPreview')}
          </label>
          <div className="flex-1 font-mono text-sm bg-bg-tertiary border border-border rounded p-3 overflow-auto whitespace-pre-wrap break-words min-h-[120px]">
            {replaceMode && replacedText ? (
              <span className="text-text-primary">{replacedText}</span>
            ) : (
              segments.map((seg, i) =>
                seg.highlight ? (
                  <span
                    key={i}
                    style={{ backgroundColor: HIGHLIGHT_COLORS[seg.colorIndex] }}
                    className="rounded-sm"
                  >
                    {seg.text}
                  </span>
                ) : (
                  <span key={i} className="text-text-secondary">
                    {seg.text}
                  </span>
                ),
              )
            )}
          </div>
        </div>
      </div>

      {/* Match results table */}
      {matches.length > 0 && (
        <div className="flex flex-col gap-2 max-h-[200px] overflow-auto">
          <label className="text-text-secondary text-xs uppercase tracking-wide font-medium">
            {t('toolkit.regex.matches')}
          </label>
          <table className="w-full text-xs font-mono border-collapse">
            <thead>
              <tr className="text-text-muted border-b border-border">
                <th className="text-left py-1.5 px-2 font-medium">#</th>
                <th className="text-left py-1.5 px-2 font-medium">Match</th>
                <th className="text-left py-1.5 px-2 font-medium">Index</th>
                <th className="text-left py-1.5 px-2 font-medium">Length</th>
                {hasGroups && (
                  <th className="text-left py-1.5 px-2 font-medium">Groups</th>
                )}
              </tr>
            </thead>
            <tbody>
              {matches.map((m, i) => (
                <tr key={i} className="border-b border-border/50 hover:bg-bg-tertiary">
                  <td className="py-1.5 px-2 text-text-muted">{i + 1}</td>
                  <td className="py-1.5 px-2 text-text-primary">
                    <span
                      className="rounded-sm px-1"
                      style={{ backgroundColor: HIGHLIGHT_COLORS[i % HIGHLIGHT_COLORS.length] }}
                    >
                      {m.text || '(empty)'}
                    </span>
                  </td>
                  <td className="py-1.5 px-2 text-text-secondary">{m.index}</td>
                  <td className="py-1.5 px-2 text-text-secondary">{m.length}</td>
                  {hasGroups && (
                    <td className="py-1.5 px-2 text-text-secondary">
                      {m.groupEntries.map((g, gi) => (
                        <span key={gi} className="mr-2">
                          ${gi + 1}: {g ?? 'undefined'}
                        </span>
                      ))}
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Named capture group table */}
      {hasNamedGroups && (
        <div className="flex flex-col gap-2 max-h-[160px] overflow-auto">
          <label className="text-text-secondary text-xs uppercase tracking-wide font-medium">
            {t('toolkit.regex.namedCaptureGroups')}
          </label>
          <table className="w-full text-xs font-mono border-collapse">
            <thead>
              <tr className="text-text-muted border-b border-border">
                <th className="text-left py-1.5 px-2 font-medium">Match #</th>
                <th className="text-left py-1.5 px-2 font-medium">Group Name</th>
                <th className="text-left py-1.5 px-2 font-medium">Value</th>
              </tr>
            </thead>
            <tbody>
              {matches.map((m, mi) =>
                Object.entries(m.groups).map(([name, value]) => (
                  <tr key={`${mi}-${name}`} className="border-b border-border/50 hover:bg-bg-tertiary">
                    <td className="py-1.5 px-2 text-text-muted">{mi + 1}</td>
                    <td className="py-1.5 px-2 text-accent-gold">{name}</td>
                    <td className="py-1.5 px-2 text-text-primary">{value ?? 'undefined'}</td>
                  </tr>
                )),
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
