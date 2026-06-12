// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/* eslint-disable i18next/no-literal-string -- technical framework documentation, not user-facing UI */

export { PasifaSection } from './FrameworkPasifa';

const SOVEREIGNTY_COMPONENTS = [
  { name: 'Build Health', weight: '15%', measures: 'Compilation and build success' },
  { name: 'Test Health', weight: '15%', measures: 'Test suite pass rate' },
  { name: 'Source Pipeline', weight: '10%', measures: 'Data source responsiveness' },
  { name: 'Dependency Freshness', weight: '10%', measures: 'Dependency currency' },
  { name: 'Invariant Compliance', weight: '15%', measures: 'System constraint satisfaction' },
  { name: 'File Size Compliance', weight: '5%', measures: 'Maintainability limits' },
  { name: 'Decision Debt', weight: '10%', measures: 'Architectural decision review status' },
  { name: 'Strategic Alignment', weight: '5%', measures: 'Dev patterns vs stated priorities' },
  { name: 'Memory Health', weight: '5%', measures: 'Knowledge utilisation ratio' },
  { name: 'Metabolism', weight: '10%', measures: 'Code vitality (hot/warm/cold/dead)' },
];

export function SectionHeading({ children }: { children: React.ReactNode }) {
  return <h3 className="text-lg font-semibold text-text-primary mb-3">{children}</h3>;
}

export function SubHeading({ children }: { children: React.ReactNode }) {
  return <h4 className="text-sm font-medium text-text-primary mt-5 mb-2">{children}</h4>;
}

export function Paragraph({ children }: { children: React.ReactNode }) {
  return <p className="text-sm text-text-secondary leading-relaxed mb-3">{children}</p>;
}

export function Highlight({ children }: { children: React.ReactNode }) {
  return <span className="text-text-primary font-medium">{children}</span>;
}

export function Card({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-bg-tertiary/50 border border-border rounded-lg p-4 mb-4">
      {children}
    </div>
  );
}

export function OverviewSection() {
  return (
    <div className="space-y-3">
      <SectionHeading>The 4DA Framework</SectionHeading>
      <Paragraph>
        4DA is a framework for <Highlight>sovereign developer intelligence</Highlight> — a system
        where content is scored, filtered, and surfaced by a machine that works exclusively for its
        operator, on its operator's hardware. There is no 4DA server: your indexed content, scores,
        and decisions never leave your machine unless you choose to use a cloud model with your own key.
      </Paragraph>
      <Paragraph>
        Three pillars form the foundation: the <Highlight>PASIFA scoring philosophy</Highlight>,
        the <Highlight>Authority Stack governance model</Highlight>, and the{' '}
        <Highlight>Autonomous Operations System (AOS)</Highlight>.
      </Paragraph>
      <Card>
        <p className="text-xs text-text-muted uppercase tracking-wider mb-3 font-medium">The Problem</p>
        <Paragraph>
          Developers receive thousands of content signals per day. Existing tools optimise for
          engagement, not relevance. The result: noise fatigue or missed critical signals.
        </Paragraph>
        <Paragraph>
          4DA's thesis: a machine that knows your codebase, your tech stack, your recent work, and
          your declared interests can filter most content and show you only what matters —
          all on your own machine, with no 4DA server in the loop. Tested at 92% rejection across 9 developer personas.
        </Paragraph>
      </Card>
      <Card>
        <p className="text-xs text-text-muted uppercase tracking-wider mb-3 font-medium">The Triad</p>
        <div className="grid grid-cols-3 gap-3 text-center">
          {[
            { name: 'PASIFA', desc: 'Multi-axis scoring with confirmation gate', color: 'text-orange-400' },
            { name: 'Authority Stack', desc: 'Constitutional governance for software', color: 'text-amber-400' },
            { name: 'AOS', desc: 'Autonomous health and operations', color: 'text-cyan-400' },
          ].map(({ name, desc, color }) => (
            <div key={name} className="bg-bg-secondary rounded-lg p-3 border border-border/50">
              <p className={`text-xs font-medium ${color} mb-1`}>{name}</p>
              <p className="text-[11px] text-text-muted">{desc}</p>
            </div>
          ))}
        </div>
        <p className="text-xs text-text-muted mt-3 text-center">
          Each system makes the others more effective. Authority Stack governs PASIFA.
          AOS maintains both. Feedback closes the loop.
        </p>
      </Card>
    </div>
  );
}

export function AuthoritySection() {
  return (
    <div className="space-y-3">
      <SectionHeading>The Authority Stack</SectionHeading>
      <Paragraph>
        A hierarchical governance framework that prevents decision re-litigation, builds
        institutional memory, and constrains AI assistants within constitutional boundaries.
      </Paragraph>
      <Card>
        <p className="text-xs text-text-muted uppercase tracking-wider mb-3 font-medium">The Hierarchy</p>
        <div className="space-y-2">
          {[
            { level: 'INVARIANTS', desc: 'What must ALWAYS or NEVER happen', color: 'text-red-400', weight: 'Highest authority' },
            { level: 'WISDOM', desc: 'How we work: principles, gates, enforcement', color: 'text-orange-400', weight: 'Governs behaviour' },
            { level: 'DECISIONS', desc: 'What was chosen and why', color: 'text-amber-400', weight: 'Prevents re-litigation' },
            { level: 'FAILURE MODES', desc: 'What breaks and how', color: 'text-yellow-400', weight: 'Institutional memory' },
          ].map(({ level, desc, color, weight }) => (
            <div key={level} className="flex items-center gap-3 py-2 px-3 bg-bg-secondary rounded-lg border border-border/50">
              <span className={`text-xs font-mono font-medium ${color} w-28 flex-shrink-0`}>{level}</span>
              <span className="text-sm text-text-secondary flex-1">{desc}</span>
              <span className="text-[10px] text-text-muted flex-shrink-0">{weight}</span>
            </div>
          ))}
        </div>
        <p className="text-xs text-text-muted mt-3">Higher always wins. A decision cannot violate an invariant.</p>
      </Card>
      <SubHeading>Seven Principles</SubHeading>
      <div className="grid grid-cols-1 gap-2">
        {[
          { id: 'W-1', name: 'Consequences Compound', desc: 'Every outcome shapes what follows.' },
          { id: 'W-2', name: 'Privacy Is Architecture', desc: 'Enforce by structure, not policy.' },
          { id: 'W-3', name: 'Trust Is Asymmetric', desc: 'One regression > ten clean commits.' },
          { id: 'W-4', name: 'Structural Impossibility', desc: 'Make it impossible, don\'t forbid.' },
          { id: 'W-5', name: 'Human Sovereignty', desc: 'AI amplifies, never replaces.' },
          { id: 'W-6', name: 'Refusal Valid, Paralysis Not', desc: 'State what you know, let the human choose.' },
          { id: 'W-7', name: 'Simplicity Is the Final Guard', desc: 'Every layer is an attack surface.' },
        ].map(({ id, name, desc }) => (
          <div key={id} className="flex items-start gap-3 py-2">
            <span className="text-[10px] font-mono text-amber-400 bg-amber-500/10 px-1.5 py-0.5 rounded flex-shrink-0">{id}</span>
            <div>
              <span className="text-sm text-text-primary">{name}</span>
              <span className="text-xs text-text-muted ms-1">— {desc}</span>
            </div>
          </div>
        ))}
      </div>
      <SubHeading>Zero Zones</SubHeading>
      <Paragraph>Structural impossibilities — no override, no emergency exception.</Paragraph>
      <div className="space-y-1">
        {['Data Exfiltration', 'Credential Exposure', 'Silent Failure', 'Self-Expanding Scope', 'Manufactured Certainty'].map(zone => (
          <div key={zone} className="flex items-center gap-2 py-1">
            <span className="w-1.5 h-1.5 rounded-full bg-red-500 flex-shrink-0" />
            <span className="text-sm text-text-secondary">{zone}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

export function AosSection() {
  return (
    <div className="space-y-3">
      <SectionHeading>AOS: Autonomous Operations System</SectionHeading>
      <Paragraph>
        A framework for measuring and maintaining system health autonomously.
      </Paragraph>
      <SubHeading>Sovereignty Score (0-100) — Design Specification</SubHeading>
      <Card>
        <table className="w-full text-xs">
          <thead>
            <tr className="text-text-muted border-b border-border/50">
              <th className="text-start py-2 font-medium">Component</th>
              <th className="text-end py-2 font-medium">Weight</th>
              <th className="text-start py-2 ps-4 font-medium">Measures</th>
            </tr>
          </thead>
          <tbody>
            {SOVEREIGNTY_COMPONENTS.map(({ name, weight, measures }) => (
              <tr key={name} className="border-b border-border/20">
                <td className="py-1.5 text-text-secondary">{name}</td>
                <td className="py-1.5 text-end font-mono text-text-secondary">{weight}</td>
                <td className="py-1.5 ps-4 text-text-muted">{measures}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
      <div className="grid grid-cols-4 gap-2">
        {[
          { range: '90-100', label: 'Sovereign', color: 'text-green-400 bg-green-500/10 border-green-500/30' },
          { range: '70-89', label: 'Healthy', color: 'text-cyan-400 bg-cyan-500/10 border-cyan-500/30' },
          { range: '50-69', label: 'Degraded', color: 'text-amber-400 bg-amber-500/10 border-amber-500/30' },
          { range: '<50', label: 'Critical', color: 'text-red-400 bg-red-500/10 border-red-500/30' },
        ].map(({ range, label, color }) => (
          <div key={range} className={`text-center rounded-lg p-2 border ${color}`}>
            <p className="text-sm font-mono font-medium">{range}</p>
            <p className="text-[10px] mt-0.5">{label}</p>
          </div>
        ))}
      </div>
      <SubHeading>Decision Delegation</SubHeading>
      <div className="space-y-2">
        {[
          { tier: 'Tier 1', name: 'Autonomous', desc: 'Build fixes, lint, test maintenance', color: 'text-green-400' },
          { tier: 'Tier 2', name: 'Recommend', desc: 'Scoring changes, architecture, features', color: 'text-amber-400' },
          { tier: 'Tier 3', name: 'Human Only', desc: 'Launch, pricing, strategy, security', color: 'text-red-400' },
        ].map(({ tier, name, desc, color }) => (
          <Card key={tier}>
            <div className="flex items-start gap-3">
              <span className={`text-xs font-mono font-medium ${color} flex-shrink-0`}>{tier}</span>
              <div>
                <span className="text-sm text-text-primary">{name}</span>
                <p className="text-xs text-text-muted mt-1">{desc}</p>
              </div>
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}

export function PrivacySection() {
  return (
    <div className="space-y-3">
      <SectionHeading>Privacy as Architecture</SectionHeading>
      <Paragraph>
        Network access is <Highlight>CSP-restricted</Highlight> to a fixed whitelist with no
        4DA-owned endpoints, so your indexed data leaves only when you explicitly send it — for
        example, to a cloud model you configured with your own key.
      </Paragraph>
      <Card>
        <p className="text-xs text-text-muted uppercase tracking-wider mb-3 font-medium">Enforcement Layers</p>
        <div className="space-y-2">
          {[
            { layer: 'CSP', mechanism: 'Network requests restricted to a whitelist. No 4DA-owned endpoints.' },
            { layer: 'Keychain', mechanism: 'API keys in OS-level secure storage. Never plaintext on disk.' },
            { layer: 'Zero Telemetry', mechanism: 'No analytics, tracking, or error reporting. Verified by audit.' },
            { layer: 'Secrets Scan', mechanism: '26 patterns in pre-commit hooks prevent credential leaks.' },
            { layer: 'Local DB', mechanism: 'SQLite on your filesystem. No cloud sync unless explicitly enabled.' },
          ].map(({ layer, mechanism }) => (
            <div key={layer} className="flex items-start gap-3 py-1">
              <span className="text-xs font-mono text-green-400 bg-green-500/10 px-2 py-0.5 rounded flex-shrink-0 w-24 text-center">{layer}</span>
              <span className="text-sm text-text-secondary">{mechanism}</span>
            </div>
          ))}
        </div>
      </Card>
      <SubHeading>Why Architecture Beats Policy</SubHeading>
      <div className="grid grid-cols-2 gap-3">
        <Card>
          <p className="text-xs font-medium text-red-400 mb-2">Policy-Based</p>
          <div className="space-y-1 text-xs text-text-muted">
            <p>Depends on trust</p><p>Fragile</p><p>Unverifiable</p><p>Terms can change</p>
          </div>
        </Card>
        <Card>
          <p className="text-xs font-medium text-green-400 mb-2">Architecture-Based</p>
          <div className="space-y-1 text-xs text-text-muted">
            <p>Depends on structure</p><p>Verifiable (read the CSP)</p><p>One-way door</p><p>Source code is the policy</p>
          </div>
        </Card>
      </div>
    </div>
  );
}

export function CompoundSection() {
  return (
    <div className="space-y-3">
      <SectionHeading>The Compound Knowledge Thesis</SectionHeading>
      <Paragraph>
        A private intelligence system compounds knowledge in ways cloud systems cannot.
      </Paragraph>
      <SubHeading>Why Local Systems Compound</SubHeading>
      <Card>
        <div className="space-y-2">
          {[
            { month: 'Month 1', state: 'System knows your declared interests' },
            { month: 'Month 3', state: 'Feedback patterns learned, scoring adjusted' },
            { month: 'Month 6', state: 'Taste embedding captures holistic preferences' },
            { month: 'Month 12', state: 'Calibration deltas correct domain-specific biases' },
          ].map(({ month, state }) => (
            <div key={month} className="flex items-center gap-3">
              <span className="text-xs font-mono text-text-muted w-16 flex-shrink-0">{month}</span>
              <div className="flex-1 h-px bg-border/50" />
              <span className="text-sm text-text-secondary">{state}</span>
            </div>
          ))}
        </div>
      </Card>
      <Paragraph>
        A competitor who clones the code gets the algorithm but not the calibration. The compound
        advantage is <Highlight>personal and non-transferable</Highlight>.
      </Paragraph>
      <SubHeading>The Network Extension</SubHeading>
      <Card>
        <p className="text-xs text-text-muted text-center">
          "Dumb relay, smart clients" — the server is intentionally stupid. Each client
          decrypts, interprets, and acts independently. Intelligence is distributed.
        </p>
      </Card>
    </div>
  );
}
