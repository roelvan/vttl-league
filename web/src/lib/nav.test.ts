import { describe, expect, it, vi } from 'vitest';

vi.mock('./viewer.svelte', () => ({ viewClub: () => club }));
let club: string | null = 'WVL154';

import { clubPath, loginUrl, matchIdFromSlug, safeReturn, urls } from './nav';

describe('urls', () => {
  it('always has the club, so one link works for everyone', () => {
    club = 'WVL154';
    expect(urls.home()).toBe('/c/WVL154');
    expect(urls.day('2026-11-20')).toBe('/c/WVL154/dag/2026-11-20');
    expect(urls.match('PWVL03/045')).toBe('/c/WVL154/wedstrijd/PWVL03-045');
    expect(urls.lineup('PWVL03/045')).toBe('/c/WVL154/wedstrijd/PWVL03-045/opstelling');
    expect(matchIdFromSlug('PWVL03-045')).toBe('PWVL03/045');
  });

  it('has no club prefix while no club is known', () => {
    club = null;
    expect(urls.home()).toBe('/');
    expect(urls.day('2026-11-20')).toBe('/dag/2026-11-20');
  });

  it('moves an old URL to the club', () => {
    expect(clubPath('Vl-B234', '/wedstrijd/X-1')).toBe('/c/Vl-B234/wedstrijd/X-1');
    expect(clubPath('WVL154', '/')).toBe('/c/WVL154');
  });

  it('returns after a login only to a path of the app', () => {
    expect(safeReturn('/c/WVL154/dag/2026-11-20')).toBe('/c/WVL154/dag/2026-11-20');
    expect(safeReturn('//evil.example')).toBe('/');
    expect(safeReturn('https://evil.example')).toBe('/');
    expect(safeReturn('/login?terug=/x')).toBe('/');
    expect(safeReturn(null)).toBe('/');
    expect(loginUrl('/c/WVL154?x=1')).toBe('/login?terug=%2Fc%2FWVL154%3Fx%3D1');
  });
});
