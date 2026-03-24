import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

interface TeamInviteDialogProps {
  onClose: () => void;
}

export function TeamInviteDialog({ onClose }: TeamInviteDialogProps) {
  const { t } = useTranslation();
  const createInvite = useAppStore(s => s.createInvite);

  const [role, setRole] = useState<'member' | 'admin'>('member');
  const [loading, setLoading] = useState(false);
  const [inviteCode, setInviteCode] = useState<string | null>(null);
  const [expiresAt, setExpiresAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement;
    const dialog = dialogRef.current;
    if (dialog) {
      const firstInput = dialog.querySelector<HTMLElement>('input, button, select, textarea');
      firstInput?.focus();
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Tab' && dialog) {
        const focusable = dialog.querySelectorAll<HTMLElement>(
          'input, button, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      previouslyFocused?.focus();
    };
  }, []);

  const handleCreate = async () => {
    setLoading(true);
    setError(null);
    const result = await createInvite(role);
    setLoading(false);

    if (result.ok && result.code) {
      setInviteCode(result.code);
      setExpiresAt(result.expiresAt ?? null);
    } else {
      setError(result.error ?? t('enterprise.invite.createFailed', 'Failed to create invite'));
    }
  };

  const handleCopy = async () => {
    if (!inviteCode) return;
    try {
      await navigator.clipboard.writeText(inviteCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback: select the text
      const el = document.querySelector<HTMLInputElement>('[data-invite-code]');
      el?.select();
    }
  };

  return (
    <div
      className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-[60] p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="invite-dialog-title"
      onClick={e => { if (e.target === e.currentTarget) onClose(); }}
      onKeyDown={e => { if (e.key === 'Escape') onClose(); }}
    >
      <div ref={dialogRef} className="bg-bg-secondary border border-border rounded-xl w-full max-w-md shadow-2xl">
        {/* Header */}
        <div className="px-5 py-4 border-b border-border flex items-center justify-between">
          <h3 id="invite-dialog-title" className="text-sm font-medium text-white">
            {t('settings.team.inviteMember', 'Invite Team Member')}
          </h3>
          <button
            onClick={onClose}
            aria-label="Close"
            className="w-7 h-7 rounded-lg bg-bg-tertiary text-text-muted hover:text-white hover:bg-border flex items-center justify-center transition-all"
          >
            &times;
          </button>
        </div>

        <div className="p-5 space-y-4">
          {!inviteCode ? (
            <>
              {/* Role Selection */}
              <div>
                <label className="text-xs text-text-muted block mb-2">
                  {t('settings.team.inviteRole', 'Role')}
                </label>
                <div className="flex gap-2" role="group" aria-label="Role selection">
                  <button
                    onClick={() => setRole('member')}
                    aria-pressed={role === 'member'}
                    className={`flex-1 px-3 py-2 text-xs rounded-lg border transition-all ${
                      role === 'member'
                        ? 'border-success/50 bg-success/10 text-success'
                        : 'border-border bg-bg-tertiary text-text-secondary hover:border-border/80'
                    }`}
                  >
                    {t('settings.team.roleMember', 'Member')}
                  </button>
                  <button
                    onClick={() => setRole('admin')}
                    aria-pressed={role === 'admin'}
                    className={`flex-1 px-3 py-2 text-xs rounded-lg border transition-all ${
                      role === 'admin'
                        ? 'border-success/50 bg-success/10 text-success'
                        : 'border-border bg-bg-tertiary text-text-secondary hover:border-border/80'
                    }`}
                  >
                    {t('settings.team.roleAdmin', 'Admin')}
                  </button>
                </div>
                <p className="text-[10px] text-text-muted mt-1.5">
                  {role === 'admin'
                    ? t('settings.team.adminDesc', 'Admins can invite members, manage settings, and configure sharing.')
                    : t('settings.team.memberDesc', 'Members can view shared signals, participate in decisions, and share their DNA.')}
                </p>
              </div>

              {/* Error */}
              {error && (
                <div className="text-xs p-2.5 rounded-lg bg-error/10 text-error border border-error/30">
                  {error}
                </div>
              )}

              {/* Create Button */}
              <button
                onClick={handleCreate}
                disabled={loading}
                className="w-full px-4 py-2.5 text-sm font-medium text-white bg-success rounded-lg hover:bg-[#1DA34D] transition-colors disabled:opacity-50"
              >
                {loading
                  ? t('settings.team.generating', 'Generating...')
                  : t('settings.team.generateInvite', 'Generate Invite Code')}
              </button>

              <p className="text-[10px] text-text-muted text-center">
                {t('settings.team.inviteExpiry', 'Invite codes expire after 72 hours and can only be used once.')}
              </p>
            </>
          ) : (
            <>
              {/* Invite Code Display */}
              <div className="text-center space-y-3">
                <div className="w-12 h-12 mx-auto rounded-full bg-success/15 flex items-center justify-center">
                  <span className="text-success text-lg">&#10003;</span>
                </div>
                <p className="text-xs text-text-secondary">
                  {t('settings.team.inviteReady', 'Share this code with your team member:')}
                </p>

                <div className="relative">
                  <input
                    data-invite-code
                    readOnly
                    value={inviteCode}
                    className="w-full px-4 py-3 bg-bg-primary border border-success/30 rounded-lg text-center text-lg font-mono text-white tracking-[0.3em] select-all focus:outline-none"
                    onClick={e => (e.target as HTMLInputElement).select()}
                  />
                </div>

                <button
                  onClick={handleCopy}
                  className="w-full px-4 py-2 text-xs font-medium text-success border border-success/30 rounded-lg hover:bg-success/10 transition-colors"
                >
                  {copied
                    ? t('settings.team.copied', 'Copied!')
                    : t('settings.team.copyCode', 'Copy to Clipboard')}
                </button>

                {expiresAt && (
                  <p className="text-[10px] text-text-muted">
                    {t('settings.team.expiresAt', 'Expires')}: {new Date(expiresAt).toLocaleString()}
                  </p>
                )}

                <p className="text-[10px] text-text-muted">
                  {t('settings.team.inviteInstructions', 'The recipient should go to Settings > Team Sync > Join Team and enter this code.')}
                </p>
              </div>

              {/* Generate Another */}
              <button
                onClick={() => { setInviteCode(null); setExpiresAt(null); setError(null); }}
                className="w-full px-4 py-2 text-xs text-text-secondary hover:text-white transition-colors"
              >
                {t('settings.team.generateAnother', 'Generate Another Invite')}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
