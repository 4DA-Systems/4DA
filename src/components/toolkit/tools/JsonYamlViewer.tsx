import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';

type Mode = 'editor' | 'tree' | 'diff';
type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

interface ParseResult {
  data: JsonValue | undefined;
  error: string | null;
}

function parseJson(raw: string): ParseResult {
  if (!raw.trim()) return { data: undefined, error: null };
  try {
    return { data: JSON.parse(raw), error: null };
  } catch (e: unknown) {
    const msg = e instanceof SyntaxError ? e.message : 'Invalid JSON';
    const posMatch = msg.match(/position\s+(\d+)/i);
    if (posMatch) {
      const pos = parseInt(posMatch[1], 10);
      let line = 1;
      let col = 1;
      for (let i = 0; i < pos && i < raw.length; i++) {
        if (raw[i] === '\n') { line++; col = 1; } else { col++; }
      }
      return { data: undefined, error: `${msg} (line ${line}, column ${col})` };
    }
    return { data: undefined, error: msg };
  }
}

function computeDiff(a: string, b: string): { type: 'same' | 'added' | 'removed'; text: string }[] {
  const linesA = a.split('\n');
  const linesB = b.split('\n');
  const result: { type: 'same' | 'added' | 'removed'; text: string }[] = [];
  const max = Math.max(linesA.length, linesB.length);
  for (let i = 0; i < max; i++) {
    const la = i < linesA.length ? linesA[i] : undefined;
    const lb = i < linesB.length ? linesB[i] : undefined;
    if (la === lb) {
      result.push({ type: 'same', text: la! });
    } else {
      if (la !== undefined) result.push({ type: 'removed', text: la });
      if (lb !== undefined) result.push({ type: 'added', text: lb });
    }
  }
  return result;
}

function copyToClipboard(text: string) {
  navigator.clipboard.writeText(text).catch(() => {});
}

function ValueDisplay({ value }: { value: JsonValue }) {
  if (value === null) return <span className="text-text-muted italic">null</span>;
  if (typeof value === 'string')
    return <span className="text-green-400">&quot;{value}&quot;</span>;
  if (typeof value === 'number')
    return <span className="text-blue-400">{String(value)}</span>;
  if (typeof value === 'boolean')
    return <span className="text-orange-400">{String(value)}</span>;
  return null;
}

function TreeNode({
  label,
  value,
  path,
  defaultOpen = false,
}: {
  label: string;
  value: JsonValue;
  path: string;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const [copied, setCopied] = useState(false);

  const handleCopyPath = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      copyToClipboard(path);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    },
    [path],
  );

  const isExpandable = value !== null && typeof value === 'object';

  if (!isExpandable) {
    return (
      <div className="flex items-center gap-1 py-0.5 pl-4 group">
        <span className="w-4" />
        <button
          onClick={handleCopyPath}
          className="text-accent-gold hover:underline cursor-pointer bg-transparent border-none font-mono text-sm"
          title={`Copy path: ${path}`}
        >
          {label}
        </button>
        <span className="text-text-muted">:</span>
        <ValueDisplay value={value} />
        {copied && <span className="text-xs text-success ml-2">copied</span>}
      </div>
    );
  }

  const isArray = Array.isArray(value);
  const entries = isArray
    ? value.map((v, i) => [String(i), v] as [string, JsonValue])
    : Object.entries(value as Record<string, JsonValue>);
  const bracket = isArray ? ['[', ']'] : ['{', '}'];

  return (
    <div className="pl-4">
      <div
        className="flex items-center gap-1 py-0.5 cursor-pointer select-none group"
        onClick={() => setOpen(!open)}
      >
        <span className="w-4 text-text-muted text-xs text-center">
          {open ? '\u25BC' : '\u25B6'}
        </span>
        <button
          onClick={handleCopyPath}
          className="text-accent-gold hover:underline cursor-pointer bg-transparent border-none font-mono text-sm"
          title={`Copy path: ${path}`}
        >
          {label}
        </button>
        <span className="text-text-muted">:</span>
        <span className="text-text-muted text-sm">
          {bracket[0]}
          {!open && <span className="mx-1">...{bracket[1]}</span>}
          <span className="text-text-muted text-xs ml-1">
            {entries.length} {isArray ? 'items' : 'keys'}
          </span>
        </span>
        {copied && <span className="text-xs text-success ml-2">copied</span>}
      </div>
      {open && (
        <div>
          {entries.map(([k, v]) => (
            <TreeNode
              key={k}
              label={k}
              value={v}
              path={isArray ? `${path}[${k}]` : `${path}.${k}`}
            />
          ))}
          <div className="pl-4 text-text-muted text-sm">{bracket[1]}</div>
        </div>
      )}
    </div>
  );
}

function ToolbarButton({
  onClick,
  children,
  active = false,
  variant = 'default',
}: {
  onClick: () => void;
  children: React.ReactNode;
  active?: boolean;
  variant?: 'default' | 'accent';
}) {
  const base =
    'px-3 py-1.5 text-xs font-medium rounded border transition-colors cursor-pointer';
  const styles =
    variant === 'accent' || active
      ? `${base} bg-accent-gold/15 text-accent-gold border-accent-gold/30 hover:bg-accent-gold/25`
      : `${base} bg-bg-tertiary text-text-secondary border-border hover:text-text-primary hover:border-[#3A3A3A]`;
  return (
    <button className={styles} onClick={onClick}>
      {children}
    </button>
  );
}

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
