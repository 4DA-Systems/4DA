// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { formatProjectNames, shortenProjectPath } from './project-path';

describe('project-path labels', () => {
  it('shortens to the last two segments across separators', () => {
    expect(shortenProjectPath('D:\\4DA\\src-tauri')).toBe('4DA/src-tauri');
    expect(shortenProjectPath('/home/me/proj')).toBe('me/proj');
    expect(shortenProjectPath('proj')).toBe('proj');
  });

  it('colliding project names render distinct labels (no "src-tauri, src-tauri")', () => {
    const labels = formatProjectNames([
      'd:/4da/src-tauri',
      'd:/work/tools/apps/bridge/src-tauri',
    ]);
    expect(labels).toEqual(['4da/src-tauri', 'bridge/src-tauri']);
    expect(new Set(labels).size).toBe(2);
  });

  it('grows a label leftwards only while it still collides', () => {
    const labels = formatProjectNames([
      'd:/work/a/app/src-tauri',
      'd:/work/b/app/src-tauri',
      'd:/solo/web',
    ]);
    expect(labels).toEqual(['a/app/src-tauri', 'b/app/src-tauri', 'solo/web']);
  });

  it('dedups the same path written two ways', () => {
    expect(formatProjectNames(['D:\\x\\y', 'D:/x/y'])).toEqual(['x/y']);
  });
});
