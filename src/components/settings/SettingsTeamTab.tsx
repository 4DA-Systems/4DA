// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { PanelErrorBoundary } from '../PanelErrorBoundary';
import { TeamSection } from './TeamSection';
import { TeamOnboardingWizard } from '../enterprise/TeamOnboardingWizard';
import { TeamSharedSources } from '../team/TeamSharedSources';
import { OrgDashboard } from '../enterprise/OrgDashboard';
import { AuditLogViewer } from '../enterprise/AuditLogViewer';
import { WebhookManager } from '../enterprise/WebhookManager';
import { PolicyEditor } from '../enterprise/PolicyEditor';
import { SsoConfigPanel } from '../enterprise/SsoConfigPanel';
import { AdminHealthDashboard } from '../enterprise/AdminHealthDashboard';
import { WebhookDocsPanel } from '../enterprise/WebhookDocsPanel';
import { DataExportPanel } from '../enterprise/DataExportPanel';
import { ConfigDiagnostics } from '../enterprise/ConfigDiagnostics';
import WaitlistSignup from '../WaitlistSignup';

interface SettingsTeamTabProps {
  tier: string;
  isTeamOrEnterprise: boolean;
  setSettingsStatus: (s: string) => void;
}

export const SettingsTeamTab = memo(function SettingsTeamTab({
  tier,
  isTeamOrEnterprise,
  setSettingsStatus,
}: SettingsTeamTabProps) {
  return (
    <div id="tabpanel-team" role="tabpanel">
      {isTeamOrEnterprise ? (
        <div className="space-y-6">
          <PanelErrorBoundary name="Team Setup Wizard">
            <TeamOnboardingWizard />
          </PanelErrorBoundary>

          <PanelErrorBoundary name="Team Sync">
            <TeamSection onStatus={setSettingsStatus} />
          </PanelErrorBoundary>

          <PanelErrorBoundary name="Shared Sources">
            <TeamSharedSources />
          </PanelErrorBoundary>

          {tier === 'enterprise' && (
            <>
              <PanelErrorBoundary name="Organization">
                <OrgDashboard />
              </PanelErrorBoundary>

              <PanelErrorBoundary name="Audit Log">
                <AuditLogViewer />
              </PanelErrorBoundary>

              <PanelErrorBoundary name="Webhooks">
                <WebhookManager />
              </PanelErrorBoundary>

              <PanelErrorBoundary name="Retention Policies">
                <PolicyEditor />
              </PanelErrorBoundary>

              <PanelErrorBoundary name="SSO">
                <SsoConfigPanel />
              </PanelErrorBoundary>

              <PanelErrorBoundary name="Admin Health">
                <AdminHealthDashboard />
              </PanelErrorBoundary>

              <PanelErrorBoundary name="Webhook Docs">
                <WebhookDocsPanel />
              </PanelErrorBoundary>
            </>
          )}

          <PanelErrorBoundary name="Data Export">
            <DataExportPanel />
          </PanelErrorBoundary>

          <PanelErrorBoundary name="Diagnostics">
            <ConfigDiagnostics />
          </PanelErrorBoundary>
        </div>
      ) : (
        <div className="space-y-6 py-2">
          <WaitlistSignup tier="team" inline />
          <div className="border-t border-border pt-6">
            <WaitlistSignup tier="enterprise" inline />
          </div>
        </div>
      )}
    </div>
  );
});
