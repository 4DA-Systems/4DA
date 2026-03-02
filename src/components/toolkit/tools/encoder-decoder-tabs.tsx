import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

import {
  isBase64,
  base64Encode,
  base64Decode,
  base64UrlDecode,
  formatExpiry,
  computeHash,
} from './encoder-decoder-utils';
import type { Direction, HashAlgo } from './encoder-decoder-utils';

export function CopyButton({ text }: { text: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    if (!text) return;
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [text]);

  return (
    <button
      onClick={handleCopy}
      disabled={!text}
      className="px-3 py-1.5 text-xs font-medium rounded-md border border-border
        bg-bg-tertiary text-text-secondary hover:text-white hover:border-white/20
        transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
    >
      {copied ? t('action.copied') : t('action.copy')}
    </button>
  );
}

export function DirectionToggle({
  direction,
  onChange,
}: {
  direction: Direction;
  onChange: (d: Direction) => void;
}) {
  return (
    <div className="flex items-center gap-1 bg-bg-primary rounded-md p-0.5 border border-border">
      {(['encode', 'decode'] as const).map((d) => (
        <button
          key={d}
          onClick={() => onChange(d)}
          className={`px-3 py-1 text-xs font-medium rounded transition-colors capitalize ${
            direction === d
              ? 'bg-bg-tertiary text-white'
              : 'text-text-muted hover:text-text-secondary'
          }`}
        >
          {d}
        </button>
      ))}
    </div>
  );
}

export function Base64Tab() {
  const { t } = useTranslation();
  const [input, setInput] = useState('');
  const [direction, setDirection] = useState<Direction>('encode');
  const [output, setOutput] = useState('');
  const [autoDetected, setAutoDetected] = useState(false);

  useEffect(() => {
    if (!input) {
      setOutput('');
      setAutoDetected(false);
      return;
    }
    const detected = isBase64(input);
    setAutoDetected(detected);
    const dir = detected && direction === 'encode' ? 'decode' : direction;
    setOutput(dir === 'encode' ? base64Encode(input) : base64Decode(input));
  }, [input, direction]);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <DirectionToggle direction={direction} onChange={setDirection} />
        {autoDetected && (
          <span className="text-xs text-text-muted">{t('toolkit.encoder.base64Detected')}</span>
        )}
      </div>
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder={t('toolkit.encoder.base64Placeholder')}
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-primary placeholder:text-text-muted resize-none
          focus:outline-none focus:border-white/20"
      />
      <div className="flex items-center justify-between">
        <span className="text-xs text-text-muted">{t('toolkit.encoder.output')}</span>
        <CopyButton text={output} />
      </div>
      <textarea
        readOnly
        value={output}
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-secondary resize-none focus:outline-none"
      />
    </div>
  );
}

export function UrlTab() {
  const { t } = useTranslation();
  const [input, setInput] = useState('');
  const [direction, setDirection] = useState<Direction>('encode');
  const [output, setOutput] = useState('');

  useEffect(() => {
    if (!input) { setOutput(''); return; }
    try {
      setOutput(
        direction === 'encode'
          ? encodeURIComponent(input)
          : decodeURIComponent(input),
      );
    } catch {
      setOutput('[Error: invalid input]');
    }
  }, [input, direction]);

  return (
    <div className="space-y-3">
      <DirectionToggle direction={direction} onChange={setDirection} />
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder={t('toolkit.encoder.urlPlaceholder')}
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-primary placeholder:text-text-muted resize-none
          focus:outline-none focus:border-white/20"
      />
      <div className="flex items-center justify-between">
        <span className="text-xs text-text-muted">{t('toolkit.encoder.output')}</span>
        <CopyButton text={output} />
      </div>
      <textarea
        readOnly
        value={output}
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-secondary resize-none focus:outline-none"
      />
    </div>
  );
}

export function JwtTab() {
  const { t } = useTranslation();
  const [input, setInput] = useState('');
  const [header, setHeader] = useState('');
  const [payload, setPayload] = useState('');
  const [expiry, setExpiry] = useState<{ text: string; expired: boolean } | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!input.trim()) {
      setHeader('');
      setPayload('');
      setExpiry(null);
      setError('');
      return;
    }
    const parts = input.trim().split('.');
    if (parts.length !== 3) {
      setError('JWT must have 3 segments separated by dots');
      setHeader('');
      setPayload('');
      setExpiry(null);
      return;
    }
    try {
      const h = JSON.parse(base64UrlDecode(parts[0]));
      const p = JSON.parse(base64UrlDecode(parts[1]));
      setHeader(JSON.stringify(h, null, 2));
      setPayload(JSON.stringify(p, null, 2));
      setExpiry(typeof p.exp === 'number' ? formatExpiry(p.exp) : null);
      setError('');
    } catch {
      setError('Failed to decode JWT');
      setHeader('');
      setPayload('');
      setExpiry(null);
    }
  }, [input]);

  return (
    <div className="space-y-3">
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder={t('toolkit.encoder.jwtPlaceholder')}
        className="w-full h-20 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-primary placeholder:text-text-muted resize-none
          focus:outline-none focus:border-white/20"
      />
      {error && <p className="text-xs text-error">{error}</p>}
      {expiry && (
        <p className={`text-xs font-medium ${expiry.expired ? 'text-error' : 'text-success'}`}>
          {expiry.text}
        </p>
      )}
      {header && (
        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-muted">{t('toolkit.encoder.header')}</span>
            <CopyButton text={header} />
          </div>
          <pre className="bg-bg-primary border border-border rounded-lg p-3 text-xs
            font-mono text-text-secondary overflow-auto max-h-40">
            {header}
          </pre>
        </div>
      )}
      {payload && (
        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-muted">{t('toolkit.encoder.payload')}</span>
            <CopyButton text={payload} />
          </div>
          <pre className="bg-bg-primary border border-border rounded-lg p-3 text-xs
            font-mono text-text-secondary overflow-auto max-h-48">
            {payload}
          </pre>
        </div>
      )}
    </div>
  );
}

export function HashTab() {
  const { t } = useTranslation();
  const [input, setInput] = useState('');
  const [algo, setAlgo] = useState<HashAlgo>('SHA-256');
  const [output, setOutput] = useState('');
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (!input) { setOutput(''); return; }

    timerRef.current = setTimeout(() => {
      computeHash(input, algo).then(setOutput).catch(() => setOutput('[Error]'));
    }, 100);

    return () => { if (timerRef.current) clearTimeout(timerRef.current); };
  }, [input, algo]);

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <span className="text-xs text-text-muted">{t('toolkit.encoder.algorithm')}</span>
        <div className="flex items-center gap-1 bg-bg-primary rounded-md p-0.5 border border-border">
          {(['SHA-256', 'SHA-512'] as const).map((a) => (
            <button
              key={a}
              onClick={() => setAlgo(a)}
              className={`px-3 py-1 text-xs font-medium rounded transition-colors ${
                algo === a
                  ? 'bg-bg-tertiary text-white'
                  : 'text-text-muted hover:text-text-secondary'
              }`}
            >
              {a}
            </button>
          ))}
        </div>
      </div>
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder={t('toolkit.encoder.hashPlaceholder')}
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-primary placeholder:text-text-muted resize-none
          focus:outline-none focus:border-white/20"
      />
      <div className="flex items-center justify-between">
        <span className="text-xs text-text-muted">
          {algo} Hash{output ? ` (${output.length} hex chars)` : ''}
        </span>
        <CopyButton text={output} />
      </div>
      <div className="bg-bg-primary border border-border rounded-lg p-3 min-h-[3rem]
        break-all text-sm font-mono text-text-secondary">
        {output || <span className="text-text-muted">{t('toolkit.encoder.hashOutputHint')}</span>}
      </div>
    </div>
  );
}
