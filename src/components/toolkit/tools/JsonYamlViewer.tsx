import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { parseJson, computeDiff, copyToClipboard } from './json-yaml-utils';
import type { Mode, ParseResult } from './json-yaml-utils';
import { TreeNode, ToolbarButton } from './json-yaml-tree';

export default function JsonYamlViewer() {
  const { t } = useTranslation();
  const [mode, setMode] = useState<Mode>('editor');
  const [input, setInput] = useState('');
  const [parsed, setParsed] = useState<ParseResult>({ data: undefined, error: null });
  const [copyFeedback, setCopyFeedback] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const [diffLeft, setDiffLeft] = useState('');
  const [diffRight, setDiffRight] = useState('');
  const [diffResult, setDiffResult] = useState<
    { type: 'same' | 'added' | 'removed'; text: string }[]
  >([]);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setParsed(parseJson(input));
    }, 150);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [input]);

  const handleFormat = useCallback(() => {
    const result = parseJson(input);
    if (result.data !== undefined) {
      const formatted = JSON.stringify(result.data, null, 2);
      setInput(formatted);
      setParsed({ data: result.data, error: null });
    } else {
      setParsed(result);
    }
  }, [input]);

  const handleMinify = useCallback(() => {
    const result = parseJson(input);
    if (result.data !== undefined) {
      const minified = JSON.stringify(result.data);
      setInput(minified);
      setParsed({ data: result.data, error: null });
    } else {
      setParsed(result);
    }
  }, [input]);

  const handleCopyOutput = useCallback(() => {
    copyToClipboard(input);
    setCopyFeedback(true);
    setTimeout(() => setCopyFeedback(false), 1200);
  }, [input]);

  const handleDiff = useCallback(() => {
    setDiffResult(computeDiff(diffLeft, diffRight));
  }, [diffLeft, diffRight]);

  return (
    <div className="h-full flex flex-col bg-bg-primary text-text-primary">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-border bg-bg-secondary">
        <div className="flex gap-1">
          {(['editor', 'tree', 'diff'] as Mode[]).map((m) => (
            <ToolbarButton
              key={m}
              onClick={() => setMode(m)}
              active={mode === m}
            >
              {m === 'editor' ? t('toolkit.jsonViewer.editor') : m === 'tree' ? t('toolkit.jsonViewer.tree') : t('toolkit.jsonViewer.diff')}
            </ToolbarButton>
          ))}
        </div>

        <div className="w-px h-5 bg-border mx-1" />

        {mode === 'editor' && (
          <div className="flex gap-1">
            <ToolbarButton onClick={handleFormat}>{t('toolkit.jsonViewer.format')}</ToolbarButton>
            <ToolbarButton onClick={handleMinify}>{t('toolkit.jsonViewer.minify')}</ToolbarButton>
            <ToolbarButton onClick={handleCopyOutput}>
              {copyFeedback ? t('action.copied') : t('action.copy')}
            </ToolbarButton>
          </div>
        )}
        {mode === 'tree' && parsed.data !== undefined && (
          <ToolbarButton onClick={handleCopyOutput}>
            {copyFeedback ? t('action.copied') : t('toolkit.jsonViewer.copySource')}
          </ToolbarButton>
        )}
        {mode === 'diff' && (
          <ToolbarButton onClick={handleDiff} variant="accent">
            {t('toolkit.jsonViewer.compare')}
          </ToolbarButton>
        )}
      </div>

      {/* Error banner */}
      {parsed.error && mode !== 'diff' && (
        <div className="px-4 py-2 bg-error/10 border-b border-error/30 text-error text-sm font-mono">
          {parsed.error}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {mode === 'editor' && (
          <textarea
            className="w-full h-full bg-transparent text-text-primary font-mono text-sm p-4 resize-none outline-none placeholder:text-text-muted"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={t('toolkit.jsonViewer.placeholder')}
            spellCheck={false}
          />
        )}

        {mode === 'tree' && (
          <div className="p-4 font-mono text-sm overflow-auto">
            {parsed.data === undefined ? (
              <p className="text-text-muted">
                {parsed.error
                  ? t('toolkit.jsonViewer.fixErrors')
                  : t('toolkit.jsonViewer.enterJson')}
              </p>
            ) : (
              <TreeNode
                label="$"
                value={parsed.data}
                path="$"
                defaultOpen
              />
            )}
          </div>
        )}

        {mode === 'diff' && (
          <div className="flex flex-col h-full">
            <div className="grid grid-cols-2 gap-0 flex-1 min-h-0">
              <div className="flex flex-col border-r border-border">
                <div className="px-3 py-1.5 text-xs text-text-muted bg-bg-secondary border-b border-border">
                  {t('toolkit.jsonViewer.left')}
                </div>
                <textarea
                  className="flex-1 w-full bg-transparent text-text-primary font-mono text-sm p-3 resize-none outline-none placeholder:text-text-muted"
                  value={diffLeft}
                  onChange={(e) => setDiffLeft(e.target.value)}
                  placeholder={t('toolkit.jsonViewer.leftPlaceholder')}
                  spellCheck={false}
                />
              </div>
              <div className="flex flex-col">
                <div className="px-3 py-1.5 text-xs text-text-muted bg-bg-secondary border-b border-border">
                  {t('toolkit.jsonViewer.right')}
                </div>
                <textarea
                  className="flex-1 w-full bg-transparent text-text-primary font-mono text-sm p-3 resize-none outline-none placeholder:text-text-muted"
                  value={diffRight}
                  onChange={(e) => setDiffRight(e.target.value)}
                  placeholder={t('toolkit.jsonViewer.rightPlaceholder')}
                  spellCheck={false}
                />
              </div>
            </div>

            {diffResult.length > 0 && (
              <div className="border-t border-border bg-bg-secondary overflow-auto max-h-64">
                <div className="px-3 py-1.5 text-xs text-text-muted border-b border-border">
                  {t('toolkit.jsonViewer.diffResult')}
                </div>
                <div className="font-mono text-sm">
                  {diffResult.map((line, i) => (
                    <div
                      key={i}
                      className={`px-3 py-0.5 ${
                        line.type === 'added'
                          ? 'bg-green-500/15 text-green-400'
                          : line.type === 'removed'
                            ? 'bg-red-500/15 text-red-400'
                            : 'text-text-secondary'
                      }`}
                    >
                      <span className="inline-block w-4 text-text-muted select-none">
                        {line.type === 'added' ? '+' : line.type === 'removed' ? '-' : ' '}
                      </span>
                      {line.text}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
