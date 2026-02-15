import { describe, it, expect } from 'vitest';
import { getContentTypeBadge } from './content-types';

describe('getContentTypeBadge', () => {
  it('returns badge for security_advisory', () => {
    const badge = getContentTypeBadge('security_advisory');
    expect(badge).not.toBeNull();
    expect(badge!.label).toBe('Security');
    expect(badge!.colorClass).toContain('red');
  });

  it('returns badge for breaking_change', () => {
    const badge = getContentTypeBadge('breaking_change');
    expect(badge).not.toBeNull();
    expect(badge!.label).toBe('Breaking');
  });

  it('returns badge for release_notes', () => {
    const badge = getContentTypeBadge('release_notes');
    expect(badge).not.toBeNull();
    expect(badge!.label).toBe('Release');
  });

  it('returns null for discussion (default type)', () => {
    expect(getContentTypeBadge('discussion')).toBeNull();
  });

  it('returns null for undefined/null', () => {
    expect(getContentTypeBadge(undefined)).toBeNull();
    expect(getContentTypeBadge(null)).toBeNull();
  });

  it('returns null for unknown types', () => {
    expect(getContentTypeBadge('unknown_type')).toBeNull();
  });

  it('returns badges for all 8 non-discussion types', () => {
    const types = [
      'security_advisory', 'breaking_change', 'release_notes', 'deep_dive',
      'tutorial', 'show_and_tell', 'question', 'hiring',
    ];
    for (const t of types) {
      const badge = getContentTypeBadge(t);
      expect(badge, `Expected badge for ${t}`).not.toBeNull();
      expect(badge!.label.length).toBeGreaterThan(0);
      expect(badge!.colorClass.length).toBeGreaterThan(0);
    }
  });
});
