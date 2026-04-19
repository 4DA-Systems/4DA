// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

type WizardStep = 'relay' | 'create' | 'invite' | 'configure' | 'done';

const STEPS: { key: WizardStep; label: string }[] = [
  { key: 'relay', label: 'Relay Setup' },
  { key: 'create', label: 'Create Team' },
  { key: 'invite', label: 'Invite Members' },
  { key: 'configure', label: 'Configure' },
  { key: 'done', label: 'Complete' },
];

const DEFAULT_RELAY = 'https://relay.4da.ai';

export function TeamOnboardingWizard() {
  const { t } = useTranslation();
  const createTeam = useAppStore(s => s.createTeam);
  const createInvite = useAppStore(s => s.createInvite);
  const teamStatus = useAppStore(s => s.teamStatus);

  const [step, setStep] = useState<WizardStep>('relay');
  const [relayUrl, setRelayUrl] = useState(DEFAULT_RELAY);
  const [displayName, setDisplayName] = useState('');
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [inviteCode, setInviteCode] = useState<string | null>(null);
  const [inviteRole, setInviteRole] = useState('member');
  const [copied, setCopied] = useState(false);

  // Skip to done if team already exists
  if (teamStatus?.enabled && step === 'relay') {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-success" />
          <span className="text-xs text-white font-medium">
            {t('enterprise.wizard.teamActive', 'Team is active')}
          </span>
        </div>
        <p className="text-[10px] text-text-muted mt-1">
          {t('enterprise.wizard.teamConfigured', 'Your team is already configured. Manage members and settings in the Team tab.')}
        </p>
      </div>
    );
  }

  const currentStepIdx = STEPS.findIndex(s => s.key === step);

  const handleCreateTeam = async () => {
    if (!displayName.trim()) return;
    setCreating(true);
    setError(null);
    const result = await createTeam(relayUrl, displayName.trim());
    setCreating(false);
    if (result.ok) {
      setStep('invite');
    } else {
      setError(result.error || 'Failed to create team');
    }
  };

  const handleGenerateInvite = async () => {
    setError(null);
    const result = await createInvite(inviteRole);
    if (result.ok && result.code) {
      setInviteCode(result.code);
    } else {
      setError(result.error || 'Failed to generate invite');
    }
  };

  const handleCopyInvite = () => {
    if (inviteCode) {
      navigator.clipboard.writeText(inviteCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border space-y-4">
      {/* Header */}
      <div>
        <h3 className="text-sm font-medium text-white">
          {t('enterprise.wizard.title', 'Team Setup Wizard')}
        </h3>
        <p className="text-[10px] text-text-muted mt-0.5">
          {t('enterprise.wizard.description', 'Set up your team in 4 steps. All communication is end-to-end encrypted.')}
        </p>
      </div>

      {/* Progress Bar */}
      <div className="flex items-center gap-1">
        {STEPS.map((s, i) => (
          <div key={s.key} className="flex items-center flex-1">
            <div className={`h-1 flex-1 rounded-full ${
              i <= currentStepIdx ? 'bg-success' : 'bg-border'
            }`} />
          </div>
        ))}
      </div>
      <div className="flex justify-between text-[9px] text-text-muted">
        {STEPS.map(s => (
          <span key={s.key} className={s.key === step ? 'text-success' : ''}>
            {s.label}
          </span>
        ))}
      </div>

      {/* Error */}
      {error && (
        <div className="px-3 py-2 bg-error/10 border border-error/30 rounded text-xs text-error">
          {error}
        </div>
      )}

      {/* Step 1: Relay */}
      {step === 'relay' && (
        <div className="space-y-3">
          <div>
            <label className="text-[10px] text-text-muted block mb-1">
              {t('enterprise.wizard.relayUrl', 'Relay Server URL')}
            </label>
            <input
              type="text"
              value={relayUrl}
              onChange={e => setRelayUrl(e.target.value)}
              className="w-full px-3 py-2 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-success/50"
              placeholder="https://relay.4da.ai"
            />
            <p className="text-[9px] text-text-muted mt-1">
              {t('enterprise.wizard.relayNote', 'Use the default managed relay or enter your self-hosted relay URL.')}
            </p>
          </div>
          <button
            onClick={() => setStep('create')}
            disabled={!relayUrl.trim()}
            className="px-4 py-2 text-xs bg-success/15 text-success rounded hover:bg-success/25 transition-colors disabled:opacity-50"
          >
            {t('action.next', 'Next')}
          </button>
        </div>
      )}

      {/* Step 2: Create Team */}
      {step === 'create' && (
        <div className="space-y-3">
          <div>
            <label className="text-[10px] text-text-muted block mb-1">
              {t('enterprise.wizard.displayName', 'Your Display Name')}
            </label>
            <input
              type="text"
              value={displayName}
              onChange={e => setDisplayName(e.target.value)}
              className="w-full px-3 py-2 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-success/50"
              placeholder="e.g. Alice"
              onKeyDown={e => e.key === 'Enter' && handleCreateTeam()}
            />
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setStep('relay')}
              className="px-3 py-2 text-xs text-text-muted hover:text-white transition-colors"
            >
              {t('action.back', 'Back')}
            </button>
            <button
              onClick={handleCreateTeam}
              disabled={!displayName.trim() || creating}
              className="px-4 py-2 text-xs bg-success/15 text-success rounded hover:bg-success/25 transition-colors disabled:opacity-50"
            >
              {creating ? t('action.creating', 'Creating...') : t('enterprise.wizard.createTeam', 'Create Team')}
            </button>
          </div>
        </div>
      )}

      {/* Step 3: Invite Members */}
      {step === 'invite' && (
        <div className="space-y-3">
          <div className="px-3 py-2.5 bg-success/10 border border-success/30 rounded">
            <p className="text-xs text-success">
              {t('enterprise.wizard.teamCreated', 'Team created successfully!')}
            </p>
          </div>

          <div className="flex items-center gap-2">
            <select
              value={inviteRole}
              onChange={e => setInviteRole(e.target.value)}
              className="px-2 py-1.5 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none"
              aria-label="Invite role"
            >
              <option value="member">{t('team.roles.member', 'Member')}</option>
              <option value="admin">{t('team.roles.admin', 'Admin')}</option>
            </select>
            <button
              onClick={handleGenerateInvite}
              className="px-3 py-1.5 text-xs bg-success/15 text-success rounded hover:bg-success/25 transition-colors"
            >
              {t('enterprise.wizard.generateInvite', 'Generate Invite Code')}
            </button>
          </div>

          {inviteCode && (
            <div className="bg-bg-primary rounded-lg p-3 border border-border/50">
              <p className="text-[10px] text-text-muted mb-1">{t('enterprise.wizard.inviteCode', 'Invite Code')}</p>
              <div className="flex items-center gap-2">
                <code className="flex-1 text-xs text-accent-gold font-mono bg-bg-tertiary px-2 py-1.5 rounded select-all">
                  {inviteCode}
                </code>
                <button
                  onClick={handleCopyInvite}
                  className="text-[10px] px-2 py-1.5 bg-border text-text-secondary rounded hover:text-white transition-colors"
                >
                  {copied ? t('action.copied', 'Copied!') : t('action.copy', 'Copy')}
                </button>
              </div>
              <p className="text-[9px] text-text-muted mt-1.5">
                {t('enterprise.wizard.inviteNote', 'Share this code securely with your team member. It expires in 24 hours.')}
              </p>
            </div>
          )}

          <div className="flex items-center gap-2">
            <button
              onClick={() => setStep('configure')}
              className="px-4 py-2 text-xs bg-success/15 text-success rounded hover:bg-success/25 transition-colors"
            >
              {inviteCode ? t('action.next', 'Next') : t('enterprise.wizard.skipInvites', 'Skip for now')}
            </button>
          </div>
        </div>
      )}

      {/* Step 4: Configure */}
      {step === 'configure' && (
        <div className="space-y-3">
          <p className="text-xs text-text-secondary">
            {t('enterprise.wizard.configureInfo', 'Your team is ready. Here are recommended next steps:')}
          </p>
          <div className="space-y-2 text-[10px] text-text-muted">
            {[
              { icon: '\u2714', text: 'Share your Developer DNA with the team for better blind spot analysis' },
              { icon: '\u2714', text: 'Enable auto-sharing for high-priority signals' },
              { icon: '\u2714', text: 'Set up webhooks for Slack/Discord notifications' },
              { icon: '\u2714', text: 'Configure retention policies for compliance' },
            ].map((item, i) => (
              <div key={i} className="flex items-start gap-2">
                <span className="text-success mt-0.5">{item.icon}</span>
                <span>{item.text}</span>
              </div>
            ))}
          </div>
          <button
            onClick={() => setStep('done')}
            className="px-4 py-2 text-xs bg-success/15 text-success rounded hover:bg-success/25 transition-colors"
          >
            {t('enterprise.wizard.finish', 'Finish Setup')}
          </button>
        </div>
      )}

      {/* Step 5: Done */}
      {step === 'done' && (
        <div className="text-center py-4 space-y-2">
          <div className="text-2xl">&#9989;</div>
          <p className="text-xs text-white font-medium">
            {t('enterprise.wizard.allDone', 'Team setup complete!')}
          </p>
          <p className="text-[10px] text-text-muted">
            {t('enterprise.wizard.doneNote', 'Your encrypted team relay is active. Members can join using invite codes.')}
          </p>
        </div>
      )}

      {/* Security Note */}
      <div className="px-3 py-2 rounded-lg bg-bg-primary border border-border/50">
        <p className="text-[9px] text-text-muted leading-relaxed">
          {t('enterprise.wizard.security', 'All team communication uses XChaCha20Poly1305 encryption. The relay server cannot read your data — it only stores and routes encrypted blobs. "Dumb relay, smart clients."')}
        </p>
      </div>
    </div>
  );
}
