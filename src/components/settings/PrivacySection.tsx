// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { setActivityTrackingEnabled } from '../../hooks/use-telemetry';

/**
 * Privacy settings — controls what data leaves the user's device AND
 * what is recorded locally.
 *
 * Exposes:
 *   • Anonymous crash reporting (Sentry) — opt-in, default OFF, sends to network
 *   • Local activity tracking — opt-in, default OFF, stays on device
 *   • LLM content level (not yet surfaced — set in onboarding)
 *
 * Design principle: every toggle here defaults to the most privacy-preserving
 * option. User must explicitly opt IN to each. Clear disclosure of scope.
 */
export function PrivacySection() {
  const { t } = useTranslation();
  const [optedIn, setOptedIn] = useState<boolean | null>(null);
  const [activityOptedIn, setActivityOptedIn] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [showDetails, setShowDetails] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const privacy = await cmd('get_privacy_config');
        if (!cancelled) {
          setOptedIn(Boolean(privacy.crash_reporting_opt_in));
          setActivityOptedIn(Boolean(privacy.activity_tracking_opt_in));
        }
      } catch {
        if (!cancelled) {
          setOptedIn(false);
          setActivityOptedIn(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleToggle = async (next: boolean) => {
    setSaving(true);
    try {
      await cmd('set_privacy_config', { crash_reporting_opt_in: next });
      setOptedIn(next);
    } catch {
      // On failure, state stays. User will see the toggle didn't move.
    } finally {
      setSaving(false);
    }
  };

  const handleActivityToggle = async (next: boolean) => {
    setSaving(true);
    try {
      await cmd('set_privacy_config', { activity_tracking_opt_in: next });
      setActivityOptedIn(next);
      // Flip runtime telemetry gate immediately — don't wait for reload.
      setActivityTrackingEnabled(next);
    } catch {
      // Toggle stays where it was.
    } finally {
      setSaving(false);
    }
  };

  if (optedIn === null || activityOptedIn === null) {
    return null; // Loading — render nothing rather than flash wrong state
  }

  return (
    <div className="bg-bg-tertiary/30 border border-border/50 rounded-xl p-4 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-white mb-1">
          {t('settings.privacy.title', 'Privacy')}
        </h3>
        <p className="text-xs text-text-muted">
          {t(
            'settings.privacy.subtitle',
            '4DA is privacy-first. Nothing leaves your device unless you explicitly enable it here.',
          )}
        </p>
      </div>

      <div className="border-t border-border/40 pt-3">
        <label className="flex items-start gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={optedIn}
            disabled={saving}
            onChange={(e) => {
              void handleToggle(e.target.checked);
            }}
            className="mt-0.5 w-4 h-4 rounded border-border bg-bg-secondary text-accent-gold focus:ring-accent-gold/50"
          />
          <div className="flex-1">
            <div className="text-sm font-medium text-white">
              {t('settings.privacy.crashReports.title', 'Send anonymous crash reports')}
            </div>
            <p className="text-xs text-text-muted mt-0.5">
              {t(
                'settings.privacy.crashReports.subtitle',
                'Helps us fix bugs. No personal data, no IP address, no API keys. You can turn this off anytime.',
              )}
            </p>
            <button
              type="button"
              onClick={() => setShowDetails((s) => !s)}
              className="text-[11px] text-accent-gold hover:underline mt-1.5"
            >
              {showDetails
                ? t('settings.privacy.crashReports.hideDetails', 'Hide details')
                : t('settings.privacy.crashReports.showDetails', 'What exactly is sent?')}
            </button>
          </div>
        </label>

        {showDetails && (
          <div className="mt-3 p-3 bg-bg-secondary/60 rounded-lg border border-border/40">
            <div className="text-[11px] text-text-secondary space-y-2">
              <div>
                <div className="font-medium text-white mb-0.5">
                  {t('settings.privacy.crashReports.sentLabel', 'What is sent')}
                </div>
                <ul className="list-disc list-inside space-y-0.5 text-text-muted">
                  <li>{t('settings.privacy.crashReports.sent.message', 'Error message and stack trace')}</li>
                  <li>{t('settings.privacy.crashReports.sent.platform', 'OS (Windows / macOS / Linux) and app version')}</li>
                  <li>{t('settings.privacy.crashReports.sent.release', 'Release build ID for tracking fixes')}</li>
                </ul>
              </div>
              <div>
                <div className="font-medium text-white mb-0.5">
                  {t('settings.privacy.crashReports.notSentLabel', 'What is NOT sent')}
                </div>
                <ul className="list-disc list-inside space-y-0.5 text-text-muted">
                  <li>{t('settings.privacy.crashReports.notSent.ip', 'Your IP address')}</li>
                  <li>{t('settings.privacy.crashReports.notSent.pii', 'Name, email, username, or any PII')}</li>
                  <li>{t('settings.privacy.crashReports.notSent.secrets', 'API keys, license keys, tokens')}</li>
                  <li>{t('settings.privacy.crashReports.notSent.files', 'File paths (usernames stripped to <user>)')}</li>
                  <li>{t('settings.privacy.crashReports.notSent.content', 'Article content, source URLs, or search queries')}</li>
                </ul>
              </div>
              <div>
                <div className="font-medium text-white mb-0.5">
                  {t('settings.privacy.crashReports.howLabel', 'How')}
                </div>
                <p className="text-text-muted">
                  {t(
                    'settings.privacy.crashReports.howDesc',
                    'Reports are sent to Sentry.io, an industry-standard error tracking service. All scrubbing happens on your device before data leaves. See our privacy policy for details.',
                  )}
                </p>
              </div>
            </div>
          </div>
        )}
      </div>

      <div className="border-t border-border/40 pt-3">
        <label className="flex items-start gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={activityOptedIn}
            disabled={saving}
            onChange={(e) => {
              void handleActivityToggle(e.target.checked);
            }}
            className="mt-0.5 w-4 h-4 rounded border-border bg-bg-secondary text-accent-gold focus:ring-accent-gold/50"
          />
          <div className="flex-1">
            <div className="text-sm font-medium text-white">
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
