import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

import {
  clamp,
  hex2rgb,
  rgb2hex,
  rgb2hsl,
  hsl2rgb,
  contrastRatio,
  loadHistory,
  addToHistory,
  copyText,
  contrastColor,
} from './color-picker-utils';
import type { RGB, HSL } from './color-picker-utils';

function Badge({ pass, label }: { pass: boolean; label: string }) {
  const { t } = useTranslation();
  return (
    <span
      className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-semibold ${
        pass ? 'bg-[#22C55E]/20 text-[#22C55E]' : 'bg-[#EF4444]/20 text-[#EF4444]'
      }`}
    >
      {label} {pass ? t('toolkit.colorPicker.pass') : t('toolkit.colorPicker.fail')}
    </span>
  );
}

function CopyBtn({ value, label }: { value: string; label: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const handleCopy = async () => {
    await copyText(value);
    setCopied(true);
    setTimeout(() => setCopied(false), 1200);
  };
  return (
    <button
      onClick={handleCopy}
      className="px-2 py-1 text-xs rounded bg-bg-tertiary border border-border text-text-secondary hover:text-white hover:border-[#444] transition-colors"
      title={t('toolkit.colorPicker.copyLabel', { label })}
    >
      {copied ? t('action.copied') : label}
    </button>
  );
}

export default function ColorPicker() {
  const { t } = useTranslation();
  const [rgb, setRgb] = useState<RGB>({ r: 74, g: 144, b: 226 });
  const [hexInput, setHexInput] = useState('#4A90E2');
  const [history, setHistory] = useState<string[]>(loadHistory);

  const hex = rgb2hex(rgb);
  const hsl = rgb2hsl(rgb);
  const hasEyeDropper = typeof window !== 'undefined' && 'EyeDropper' in window;

  const whiteContrast = contrastRatio(rgb, { r: 255, g: 255, b: 255 });
  const blackContrast = contrastRatio(rgb, { r: 0, g: 0, b: 0 });

  useEffect(() => {
    setHexInput(hex);
  }, [hex]);

  const updateFromRgb = useCallback((next: RGB) => {
    setRgb(next);
    setHistory((prev) => addToHistory(rgb2hex(next), prev));
  }, []);

  const handleHexChange = (val: string) => {
    setHexInput(val);
    const parsed = hex2rgb(val);
    if (parsed) {
      setRgb(parsed);
      setHistory((prev) => addToHistory(rgb2hex(parsed), prev));
    }
  };

  const handleRgbChange = (channel: keyof RGB, val: string) => {
    const n = parseInt(val, 10);
    if (isNaN(n)) return;
    const next = { ...rgb, [channel]: clamp(n, 0, 255) };
    updateFromRgb(next);
  };

  const handleHslChange = (channel: keyof HSL, val: string) => {
    const n = parseInt(val, 10);
    if (isNaN(n)) return;
    const maxVal = channel === 'h' ? 360 : 100;
    const nextHsl = { ...hsl, [channel]: clamp(n, 0, maxVal) };
    updateFromRgb(hsl2rgb(nextHsl));
  };

  const handleHueSlider = (e: React.ChangeEvent<HTMLInputElement>) => {
    const h = parseInt(e.target.value, 10);
    updateFromRgb(hsl2rgb({ ...hsl, h }));
  };

  const handleEyeDropper = async () => {
    if (!hasEyeDropper) return;
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any -- EyeDropper API has no standard type
      const dropper = new (window as any).EyeDropper();
      const result = await dropper.open();
      const parsed = hex2rgb(result.sRGBHex);
      if (parsed) updateFromRgb(parsed);
    } catch { /* user cancelled */ }
  };

  const handleSwatchClick = (h: string) => {
    const parsed = hex2rgb(h);
    if (parsed) {
      setRgb(parsed);
      setHexInput(h);
    }
  };

  const rgbString = `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`;
  const hslString = `hsl(${hsl.h}, ${hsl.s}%, ${hsl.l}%)`;
  const overlayColor = contrastColor(rgb);

  return (
    <div className="flex flex-col gap-5 p-4 max-w-md">
      {/* Preview swatch */}
      <div
        className="relative w-[120px] h-[120px] rounded-lg border border-border flex items-end justify-center"
        style={{ backgroundColor: hex }}
      >
        <span
          className="font-mono text-sm font-semibold pb-2 select-all"
          style={{ color: overlayColor }}
        >
          {hex}
        </span>
      </div>

      {/* Hue slider */}
      <div className="flex flex-col gap-1">
        <label className="text-xs text-[#666] font-medium">{t('toolkit.colorPicker.hue')}</label>
        <input
          type="range"
          min={0}
          max={360}
          value={hsl.h}
          onChange={handleHueSlider}
          className="w-full h-3 rounded-full appearance-none cursor-pointer"
          style={{
            background: `linear-gradient(to right,
              hsl(0,100%,50%), hsl(60,100%,50%), hsl(120,100%,50%),
              hsl(180,100%,50%), hsl(240,100%,50%), hsl(300,100%,50%), hsl(360,100%,50%))`,
          }}
        />
      </div>

      {/* HEX input */}
      <div className="flex flex-col gap-1">
        <div className="flex items-center justify-between">
          <label className="text-xs text-[#666] font-medium">HEX</label>
          <CopyBtn value={hex} label="HEX" />
        </div>
        <input
          type="text"
          value={hexInput}
          onChange={(e) => handleHexChange(e.target.value)}
          className="font-mono text-sm bg-bg-tertiary border border-border rounded px-3 py-1.5 text-white focus:outline-none focus:border-[#444] w-full"
          spellCheck={false}
          maxLength={7}
        />
      </div>

      {/* RGB inputs */}
      <div className="flex flex-col gap-1">
        <div className="flex items-center justify-between">
          <label className="text-xs text-[#666] font-medium">RGB</label>
          <CopyBtn value={rgbString} label="RGB" />
        </div>
        <div className="flex gap-2">
          {(['r', 'g', 'b'] as const).map((ch) => (
            <div key={ch} className="flex-1 flex flex-col gap-0.5">
              <span className="text-[10px] text-[#666] uppercase">{ch}</span>
              <input
                type="number"
                min={0}
                max={255}
                value={rgb[ch]}
                onChange={(e) => handleRgbChange(ch, e.target.value)}
                className="font-mono text-sm bg-bg-tertiary border border-border rounded px-2 py-1.5 text-white focus:outline-none focus:border-[#444] w-full"
              />
            </div>
          ))}
        </div>
      </div>

      {/* HSL inputs */}
      <div className="flex flex-col gap-1">
        <div className="flex items-center justify-between">
          <label className="text-xs text-[#666] font-medium">HSL</label>
          <CopyBtn value={hslString} label="HSL" />
        </div>
        <div className="flex gap-2">
          {(['h', 's', 'l'] as const).map((ch) => (
            <div key={ch} className="flex-1 flex flex-col gap-0.5">
              <span className="text-[10px] text-[#666] uppercase">
                {ch}{ch !== 'h' ? '%' : '\u00B0'}
              </span>
              <input
                type="number"
                min={0}
                max={ch === 'h' ? 360 : 100}
                value={hsl[ch]}
                onChange={(e) => handleHslChange(ch, e.target.value)}
                className="font-mono text-sm bg-bg-tertiary border border-border rounded px-2 py-1.5 text-white focus:outline-none focus:border-[#444] w-full"
              />
            </div>
          ))}
        </div>
      </div>

      {/* EyeDropper */}
      {hasEyeDropper && (
        <button
          onClick={handleEyeDropper}
          className="w-full py-2 text-sm rounded bg-bg-tertiary border border-border text-text-secondary hover:text-white hover:border-[#444] transition-colors"
        >
          {t('toolkit.colorPicker.pickFromScreen')}
        </button>
      )}

      {/* WCAG Contrast */}
      <div className="flex flex-col gap-2 p-3 rounded-lg bg-bg-secondary border border-border">
        <span className="text-xs text-[#666] font-medium">{t('toolkit.colorPicker.wcagContrast')}</span>
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2 flex-wrap">
            <div className="w-5 h-5 rounded border border-border bg-white" />
            <span className="text-xs text-text-secondary font-mono w-12">
              {whiteContrast.toFixed(2)}
            </span>
            <Badge pass={whiteContrast >= 4.5} label="AA" />
            <Badge pass={whiteContrast >= 3.0} label="AA lg" />
            <Badge pass={whiteContrast >= 7.0} label="AAA" />
            <Badge pass={whiteContrast >= 4.5} label="AAA lg" />
          </div>
          <div className="flex items-center gap-2 flex-wrap">
            <div className="w-5 h-5 rounded border border-border bg-black" />
            <span className="text-xs text-text-secondary font-mono w-12">
              {blackContrast.toFixed(2)}
            </span>
            <Badge pass={blackContrast >= 4.5} label="AA" />
            <Badge pass={blackContrast >= 3.0} label="AA lg" />
            <Badge pass={blackContrast >= 7.0} label="AAA" />
            <Badge pass={blackContrast >= 4.5} label="AAA lg" />
          </div>
        </div>
      </div>

      {/* Color history */}
      {history.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <span className="text-xs text-[#666] font-medium">{t('toolkit.colorPicker.history')}</span>
          <div className="flex gap-1.5 flex-wrap">
            {history.map((h) => (
              <button
                key={h}
                onClick={() => handleSwatchClick(h)}
                title={h}
                className="w-7 h-7 rounded border border-border hover:border-[#444] transition-colors cursor-pointer"
                style={{ backgroundColor: h }}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
