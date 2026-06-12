// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/* eslint-disable i18next/no-literal-string -- technical framework documentation, not user-facing UI */

import { Card, Highlight, Paragraph, SectionHeading, SubHeading } from './FrameworkSections';

const GATE_TABLE = [
  { axes: 0, mult: 0.25, ceiling: 0.20 },
  { axes: 1, mult: 0.45, ceiling: 0.28 },
  { axes: 2, mult: 1.00, ceiling: 0.65 },
  { axes: 3, mult: 1.10, ceiling: 0.85 },
  { axes: 4, mult: 1.20, ceiling: 1.00 },
  { axes: 5, mult: 1.25, ceiling: 1.00 },
];

export function PasifaSection() {
  return (
    <div className="space-y-3">
      <SectionHeading>PASIFA: Privacy-Aware Scoring Intelligence</SectionHeading>
      <Paragraph>
        <Highlight>Privacy-Aware Semantic Intelligence Framework for Analysis.</Highlight>{' '}
        PASIFA is not a machine learning model — it is a scoring philosophy: a structured
        methodology for determining whether content is relevant to a specific developer,
        using only information that exists locally.
      </Paragraph>
      <SubHeading>The Five Axes</SubHeading>
      <div className="space-y-2">
        {[
          { axis: 'Context', question: 'Does this relate to what I\'m working on right now?', source: 'Local project scanning, recent git activity' },
          { axis: 'Interest', question: 'Does this match my declared or inferred interests?', source: 'User-declared topics + learned affinities' },
          { axis: 'ACE', question: 'Does this match my stack or active topics?', source: 'Dependency analysis, file patterns, git history, semantic signals' },
          { axis: 'Dependency', question: 'Is this about a library I actually depend on?', source: 'Package manifest analysis' },
          { axis: 'Learned', question: 'Has my past behaviour indicated this is valuable?', source: 'Feedback signals (saves, dismissals, time-on-content)' },
        ].map(({ axis, question, source }) => (
          <Card key={axis}>
            <div className="flex items-start gap-3">
              <span className="text-xs font-mono text-orange-400 bg-orange-500/10 px-2 py-0.5 rounded flex-shrink-0">{axis}</span>
              <div>
                <p className="text-sm text-text-primary">{question}</p>
                <p className="text-xs text-text-muted mt-1">{source}</p>
              </div>
            </div>
          </Card>
        ))}
      </div>
      <SubHeading>The Confirmation Gate</SubHeading>
      <Paragraph>
        Raw scores are not enough. PASIFA requires <Highlight>confirmation</Highlight>: at least
        two independent axes must agree that content is relevant before it is surfaced.
      </Paragraph>
      <Card>
        <table className="w-full text-xs">
          <thead>
            <tr className="text-text-muted border-b border-border/50">
              <th className="text-start py-2 font-medium">Confirming Axes</th>
              <th className="text-end py-2 font-medium">Multiplier</th>
              <th className="text-end py-2 font-medium">Ceiling</th>
              <th className="text-end py-2 font-medium">Outcome</th>
            </tr>
          </thead>
          <tbody>
            {GATE_TABLE.map(({ axes, mult, ceiling }) => (
              <tr key={axes} className="border-b border-border/20">
                <td className="py-1.5 text-text-secondary">{axes}</td>
                <td className="py-1.5 text-end font-mono text-text-secondary">{mult.toFixed(2)}</td>
                <td className="py-1.5 text-end font-mono text-text-secondary">{ceiling.toFixed(2)}</td>
                <td className="py-1.5 text-end">
                  {axes < 2 ? <span className="text-red-400">Rejected</span>
                    : axes < 4 ? <span className="text-amber-400">Confirmed</span>
                    : <span className="text-green-400">Strong</span>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
      <SubHeading>V2 Pipeline (8 Phases)</SubHeading>
      <div className="space-y-1">
        {[
          { phase: 1, name: 'Signal Extraction', desc: 'Extract raw values for all five axes' },
          { phase: 2, name: 'KNN Calibration', desc: 'Sigmoid calibration to suppress distance noise' },
          { phase: 3, name: 'Gate Count', desc: 'Count confirming signals before combination' },
          { phase: 4, name: 'Semantic Integration', desc: 'Multiplicative: base * (1 + semantic_boost)' },
          { phase: 5, name: 'Quality Composite', desc: 'Source authority, recency, domain quality' },
          { phase: 6, name: 'Boost Application', desc: 'Feedback/affinity/taste boosts, capped [-0.15, 0.35]' },
          { phase: 7, name: 'Confirmation Gate', desc: 'Apply gate table, enforce score ceiling' },
          { phase: 8, name: 'Final Threshold', desc: 'Compare against auto-tuning relevance threshold' },
        ].map(({ phase, name, desc }) => (
          <div key={phase} className="flex items-start gap-3 py-2 border-b border-border/20">
            <span className="text-xs font-mono text-text-muted w-4 flex-shrink-0">{phase}</span>
            <div>
              <span className="text-sm text-text-primary">{name}</span>
              <span className="text-xs text-text-muted ms-2">{desc}</span>
            </div>
          </div>
        ))}
      </div>
      <SubHeading>Design Decisions</SubHeading>
      <Card>
        <div className="space-y-3 text-sm text-text-secondary">
          <p><Highlight>Why five axes, not one?</Highlight> A single relevance score is fragile. Five axes provide triangulation — like GPS needing multiple satellites.</p>
          <p><Highlight>Why a confirmation gate, not a weighted sum?</Highlight> A weighted sum allows one extreme axis to overwhelm four irrelevant ones. The gate requires independent agreement.</p>
          <p><Highlight>Why precision over recall?</Highlight> The cost of false positives is asymmetrically higher than false negatives. Wasted attention cannot be recovered.</p>
        </div>
      </Card>
    </div>
  );
}
