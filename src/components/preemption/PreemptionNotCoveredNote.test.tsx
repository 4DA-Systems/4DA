// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';

import { PreemptionNotCoveredNote } from './PreemptionNotCoveredNote';

// Audit A2/E4 (2026-10-12): a Maven or .NET project was never reported as
// unsupported; a short or empty worklist read as covering it.
describe('PreemptionNotCoveredNote', () => {
  it('names the files 4DA does not read versions from', () => {
    render(
      <PreemptionNotCoveredNote
        notCovered={{
          count: 7,
          files: [
            { path: 'projects/spring-petclinic/pom.xml', ecosystem: 'Maven' },
            { path: 'apps/api/build.gradle.kts', ecosystem: 'Gradle' },
            { path: 'svc/packages.lock.json', ecosystem: 'NuGet lockfile' },
            { path: 'a/pom.xml', ecosystem: 'Maven' },
            { path: 'b/pom.xml', ecosystem: 'Maven' },
            { path: 'c/pom.xml', ecosystem: 'Maven' },
          ],
          unreadable_count: 1,
          unreadable: [{ path: 'web/package-lock.json', reason: 'invalid JSON' }],
        }}
      />,
    );
    expect(screen.getByTestId('preemption-not-covered')).toBeInTheDocument();
    expect(screen.getByText('preemption.notCovered.title')).toBeInTheDocument();
    expect(screen.getByText('Maven: projects/spring-petclinic/pom.xml')).toBeInTheDocument();
    expect(screen.getByText('Gradle: apps/api/build.gradle.kts')).toBeInTheDocument();
    // Five listed, the rest counted.
    expect(screen.queryByText('Maven: c/pom.xml')).toBeNull();
    expect(screen.getByText('preemption.notCovered.more')).toBeInTheDocument();
    // A lockfile that failed to read is named too.
    expect(screen.getByTestId('preemption-unreadable')).toBeInTheDocument();
    expect(screen.getByText('web/package-lock.json: invalid JSON')).toBeInTheDocument();
  });

  it('renders nothing when every file is covered', () => {
    const { container } = render(<PreemptionNotCoveredNote notCovered={undefined} />);
    expect(container).toBeEmptyDOMElement();
    const zero = render(
      <PreemptionNotCoveredNote notCovered={{ count: 0, files: [], unreadable_count: 0, unreadable: [] }} />,
    );
    expect(zero.container).toBeEmptyDOMElement();
  });
});
