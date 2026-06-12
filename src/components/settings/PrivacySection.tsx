// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { setActivityTrackingEnabled } from '../../hooks/use-telemetry';

/**
 * Privacy settings — controls what is recorded locally and lets the user
 * export a local diagnostic bundle on demand.
 *
 * 4DA has NO third-party crash reporting and NO telemetry. There is no
 * "send anonymous reports" toggle because nothing is ever sent automatically.
 * Instead:
 *   • Export diagnostics — builds a scrubbed, local report the user reviews
 *     and chooses whether to attach to a bug report. Nothing leaves the device.
 *   • Local activity tracking — opt-in, default OFF, stays on device, powers
 *     relevance learning.
 *
 * Design principle: every option here defaults to the most privacy-preserving
 * state. Sharing is always an explicit, per-incident user action.
 */
export function PrivacySection() {
  const { t } = useTranslation();
  const [activityOptedIn, setActivityOptedIn] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [showDetails, setShowDetails] = useState(false);

  // Diagnostics export state.
  const [exporting, setExporting] = useState(false);
  const [report, setReport] = useState<string | null>(null);
  const [savedPath, setSavedPath] = useState<string>('');
  const [copied, setCopied] = useState(false);
  const [exportError, setExportError] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const privacy = await cmd('get_privacy_config');
        if (!cancelled) {
          setActivityOptedIn(Boolean(privacy.activity_tracking_opt_in));
        }
      } catch {
        if (!cancelled) {
          setActivityOptedIn(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleActivityToggle = async (next: boolean) => {
    setSaving(true);
    try {
      await cmd('set_privacy_config', { activityTrackingOptIn: next });
      setActivityOptedIn(next);
      // Flip runtime telemetry gate immediately — don't wait for reload.
      setActivityTrackingEnabled(next);
    } catch {
      // Toggle stays where it was.
    } finally {
      setSaving(false);
    }
  };

  const handleExport = async () => {
    setExporting(true);
    setExportError(false);
    setCopied(false);
    try {
      const result = await cmd('export_diagnostics');
      setReport(result.report);
      setSavedPath(result.saved_path);
    } catch {
      setExportError(true);
    } finally {
      setExporting(false);
    }
  };

  const handleCopy = async () => {
    if (!report) return;
    try {
      await navigator.clipboard.writeText(report);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable — the user can still read/select the text.
    }
  };

  if (activityOptedIn === null) {
    return null; // Loading — render nothing rather than flash wrong state
  }

  return (
    <div className="bg-bg-tertiary/30 border border-border/50 rounded-xl p-4 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-text-primary mb-1">
          {t('settings.privacy.title', 'Privacy')}
        </h3>
        <p className="text-xs text-text-muted">
          {t(
            'settings.privacy.subtitle',
            '4DA sends no telemetry and no crash reports. Nothing leaves your device automatically — sharing is always your explicit choice.',
          )}
        </p>
      </div>

      {/* Local diagnostics export — replaces third-party crash reporting */}
      <div className="border-t border-border/40 pt-3">
        <div className="text-sm font-medium text-text-primary">
          {t('settings.privacy.diagnostics.title', 'Export diagnostics')}
        </div>
        <p className="text-xs text-text-muted mt-0.5">
          {t(
            'settings.privacy.diagnostics.subtitle',
            'If something breaks, generate a local diagnostic report — a scrubbed snapshot of app health and recent logs. Nothing is sent automatically; you review it and choose whether to attach it to a bug report.',
          )}
        </p>

        <div className="flex items-center gap-3 mt-2">
          <button
            type="button"
            onClick={() => void handleExport()}
            disabled={exporting}
            className="text-xs px-3 py-1.5 rounded-lg bg-bg-secondary border border-border hover:border-accent-gold/50 text-text-primary disabled:opacity-50"
          >
            {exporting
              ? t('settings.privacy.diagnostics.generating', 'Generating…')
              : t('settings.privacy.diagnostics.generate', 'Generate diagnostic report')}
          </button>
          <button
            type="button"
            onClick={() => setShowDetails((s) => !s)}
            className="text-[11px] text-accent-gold hover:underline"
          >
            {showDetails
              ? t('settings.privacy.diagnostics.hideDetails', 'Hide details')
              : t('settings.privacy.diagnostics.showDetails', "What's included?")}
          </button>
        </div>

        {showDetails && (
          <div className="mt-3 p-3 bg-bg-secondary/60 rounded-lg border border-border/40">
            <div className="text-[11px] text-text-secondary space-y-2">
              <div>
                <div className="font-medium text-text-primary mb-0.5">
                  {t('settings.privacy.diagnostics.includedLabel', "What's included")}
                </div>
                <ul className="list-disc list-inside space-y-0.5 text-text-muted">
                  <li>{t('settings.privacy.diagnostics.included.health', 'App version, OS, uptime, and database size')}</li>
                  <li>{t('settings.privacy.diagnostics.included.counts', 'Item / context / feedback counts and source health')}</li>
                  <li>{t('settings.privacy.diagnostics.included.logs', 'The last 400 lines of the local log')}</li>
                </ul>
              </div>
              <div>
                <div className="font-medium text-text-primary mb-0.5">
                  {t('settings.privacy.diagnostics.scrubbedLabel', "What's scrubbed")}
                </div>
                <ul className="list-disc list-inside space-y-0.5 text-text-muted">
                  <li>{t('settings.privacy.diagnostics.scrubbed.usernames', 'Usernames in file paths (replaced with <user>)')}</li>
                  <li>{t('settings.privacy.diagnostics.scrubbed.secrets', 'API keys, license keys, and tokens')}</li>
                </ul>
              </div>
              <p className="text-text-muted">
                {t(
                  'settings.privacy.diagnostics.howDesc',
                  'The report is written to a file on your machine. Nothing is transmitted. Attach it to a GitHub issue or email only if you choose to.',
                )}
              </p>
            </div>
          </div>
        )}

        {exportError && (
          <p className="text-xs text-error mt-2">
            {t('settings.privacy.diagnostics.error', 'Could not generate the report. Please try again.')}
          </p>
        )}

        {report && (
          <div className="mt-3 space-y-2">
            {savedPath && (
              <div className="text-[11px] text-text-muted break-all">
                {t('settings.privacy.diagnostics.savedTo', 'Saved to:')} <span className="text-text-secondary">{savedPath}</span>
              </div>
            )}
            <textarea
              readOnly
              value={report}
              aria-label={t('settings.privacy.diagnostics.reportLabel', 'Diagnostic report')}
              className="w-full h-40 text-[10px] font-mono bg-bg-secondary/60 border border-border/40 rounded-lg p-2 text-text-secondary resize-y"
            />
            <button
              type="button"
              onClick={() => void handleCopy()}
              className="text-xs px-3 py-1.5 rounded-lg bg-bg-secondary border border-border hover:border-accent-gold/50 text-text-primary"
            >
              {copied
                ? t('settings.privacy.diagnostics.copied', 'Copied')
                : t('settings.privacy.diagnostics.copy', 'Copy to clipboard')}
            </button>
          </div>
        )}
      </div>

      <div className="border-t border-border/40 pt-3">
        <label htmlFor="privacy-activity-tracking" aria-label={t('settings.privacy.activityTracking.title', 'Local activity tracking')} className="flex items-start gap-3 cursor-pointer">
          <input
            id="privacy-activity-tracking"
            type="checkbox"
            checked={activityOptedIn}
            disabled={saving}
            onChange={(e) => {
              void handleActivityToggle(e.target.checked);
            }}
            className="mt-0.5 w-4 h-4 rounded border-border bg-bg-secondary text-accent-gold focus:ring-accent-gold/50"
          />
          <div className="flex-1">
            <div className="text-sm font-medium text-text-primary">
              {t('settings.privacy.activityTracking.title', 'Local activity tracking')}
            </div>
            <p className="text-xs text-text-muted mt-0.5">
              {t(
                'settings.privacy.activityTracking.subtitle',
                'Record tab opens, view durations, and search queries on your device to power relevance learning. Nothing is transmitted — data stays in your local SQLite. Off by default.',
              )}
            </p>
          </div>
        </label>
      </div>
    </div>
  );
}
