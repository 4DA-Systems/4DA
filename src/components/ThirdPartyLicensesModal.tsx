import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  onClose: () => void;
}

// NOTICE file embedded as a constant. Generated once at build time from the
// repo's NOTICE file. Apache 2.0 and FSL compliance requires these
// attributions be accessible to users. This component displays them in-app.
//
// If NOTICE file changes, update this constant. Consider wiring to a build
// step that reads NOTICE at compile time (Vite raw import) — left as manual
// for now to keep the build graph simple.
const NOTICE_TEXT = `4DA Home
Copyright 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841)

Licensed under the Functional Source License, Version 1.1, Apache 2.0 Future
License (FSL-1.1-Apache-2.0). See LICENSE for details.

This product includes software developed by third parties. The following is
a non-exhaustive list of major dependencies and their licenses. Full license
texts are available in the respective source repositories.

═══════════════════════════════════════════════════════════════════════════
Rust Backend Dependencies
═══════════════════════════════════════════════════════════════════════════

tauri (Apache 2.0 / MIT) — https://github.com/tauri-apps/tauri
tokio (MIT) — https://github.com/tokio-rs/tokio
serde, serde_json (MIT / Apache 2.0) — https://github.com/serde-rs/serde
rusqlite (MIT) — https://github.com/rusqlite/rusqlite
sqlite-vec (Apache 2.0 / MIT) — https://github.com/asg017/sqlite-vec
reqwest (MIT / Apache 2.0) — https://github.com/seanmonstar/reqwest
tracing, tracing-subscriber (MIT) — https://github.com/tokio-rs/tracing
tracing-appender (MIT) — https://github.com/tokio-rs/tracing
anyhow, thiserror (MIT / Apache 2.0) — https://github.com/dtolnay/anyhow
parking_lot (MIT / Apache 2.0) — https://github.com/Amanieu/parking_lot
once_cell (MIT / Apache 2.0) — https://github.com/matklad/once_cell
ed25519-dalek (BSD 3-Clause) — https://github.com/dalek-cryptography/ed25519-dalek
chacha20poly1305 (Apache 2.0 / MIT) — https://github.com/RustCrypto/AEADs
blake3 (CC0 1.0 / Apache 2.0) — https://github.com/BLAKE3-team/BLAKE3
semver (MIT / Apache 2.0) — https://github.com/dtolnay/semver
url (MIT / Apache 2.0) — https://github.com/servo/rust-url
chrono (MIT / Apache 2.0) — https://github.com/chronotope/chrono
uuid (Apache 2.0 / MIT) — https://github.com/uuid-rs/uuid
regex (MIT / Apache 2.0) — https://github.com/rust-lang/regex
ocrs (MPL 2.0 — isolated library, not linked into final binary static layout)
pdf-extract (Apache 2.0) — https://github.com/jrmuizel/pdf-extract
lopdf (MIT) — https://github.com/J-F-Liu/lopdf
docx-rs (Apache 2.0 / MIT) — https://github.com/bokuweb/docx-rs
calamine (Apache 2.0 / MIT) — https://github.com/tafia/calamine
notify (Apache 2.0 / MIT) — https://github.com/notify-rs/notify
ts-rs (MIT) — https://github.com/Aleph-Alpha/ts-rs
keyring (Apache 2.0 / MIT) — https://github.com/hwchen/keyring-rs
whichlang (MIT) — https://github.com/quickwit-oss/whichlang
uhlc (EPL-2.0 / Apache 2.0) — https://github.com/atolab/uhlc-rs

═══════════════════════════════════════════════════════════════════════════
Frontend Dependencies
═══════════════════════════════════════════════════════════════════════════

react, react-dom (MIT) — https://github.com/facebook/react
zustand (MIT) — https://github.com/pmndrs/zustand
i18next, react-i18next (MIT) — https://github.com/i18next/i18next
@tauri-apps/api (Apache 2.0 / MIT) — https://github.com/tauri-apps/tauri
@tauri-apps/plugin-opener (Apache 2.0 / MIT)
@tauri-apps/plugin-updater (Apache 2.0 / MIT)
@fontsource-variable/inter (OFL-1.1) — https://fontsource.org
@fontsource-variable/jetbrains-mono (OFL-1.1) — https://fontsource.org
@tanstack/react-virtual (MIT) — https://github.com/TanStack/virtual
dompurify (Apache 2.0 / MPL 2.0) — https://github.com/cure53/DOMPurify
tailwindcss (MIT) — https://github.com/tailwindlabs/tailwindcss
vite (MIT) — https://github.com/vitejs/vite
typescript (Apache 2.0) — https://github.com/microsoft/TypeScript

═══════════════════════════════════════════════════════════════════════════
Full License Texts
═══════════════════════════════════════════════════════════════════════════

The complete NOTICE file with full attribution is available at:
https://github.com/runyourempire/4DA/blob/main/NOTICE

Full license texts for each dependency are available in their respective
source repositories. This list is provided to satisfy Apache 2.0 Section 4
attribution requirements and FSL-1.1-Apache-2.0 transparency obligations.

For questions about third-party licenses, contact: legal@4da.ai
`;

export function ThirdPartyLicensesModal({ onClose }: Props) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onClose]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(NOTICE_TEXT);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API can fail in some contexts — silently ignore
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="third-party-licenses-title"
    >
      <button
        type="button"
        aria-label={t('about.close', 'Close')}
        className="absolute inset-0 bg-black/70 backdrop-blur-sm border-0 cursor-default"
        onClick={onClose}
      />
      <div className="relative w-full max-w-3xl max-h-[85vh] bg-bg-secondary border border-border rounded-xl shadow-2xl flex flex-col">
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2
            id="third-party-licenses-title"
            className="text-sm font-semibold text-white"
          >
            {t('about.thirdPartyLicenses', 'Third-Party Licenses')}
          </h2>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleCopy}
              className="text-xs px-2.5 py-1 text-text-secondary bg-bg-tertiary border border-border rounded hover:bg-bg-primary transition-colors"
            >
              {copied ? t('about.copied', 'Copied') : t('about.copyToClipboard', 'Copy')}
            </button>
            <button
              type="button"
              onClick={onClose}
              className="text-xs px-2.5 py-1 text-text-secondary bg-bg-tertiary border border-border rounded hover:bg-bg-primary transition-colors"
              aria-label={t('about.close', 'Close')}
            >
              {t('about.close', 'Close')}
            </button>
          </div>
        </div>
        <pre className="flex-1 overflow-auto px-5 py-4 text-[11px] leading-relaxed text-text-secondary font-mono whitespace-pre-wrap">
          {NOTICE_TEXT}
        </pre>
      </div>
    </div>
  );
}
