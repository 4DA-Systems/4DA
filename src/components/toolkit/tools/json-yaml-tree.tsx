import { useState, useCallback } from 'react';

import { copyToClipboard } from './json-yaml-utils';
import type { JsonValue } from './json-yaml-utils';

export function ValueDisplay({ value }: { value: JsonValue }) {
  if (value === null) return <span className="text-text-muted italic">null</span>;
  if (typeof value === 'string')
    return <span className="text-green-400">&quot;{value}&quot;</span>;
  if (typeof value === 'number')
    return <span className="text-blue-400">{String(value)}</span>;
  if (typeof value === 'boolean')
    return <span className="text-orange-400">{String(value)}</span>;
  return null;
}

export function TreeNode({
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

export function ToolbarButton({
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
