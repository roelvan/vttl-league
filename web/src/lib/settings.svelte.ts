// Per-device preferences, kept in local storage.

import type { SlipLayout } from './sheet/print';

interface Settings {
  /** Dark is the default (design); the sun button switches. */
  theme: 'dark' | 'light';
  slipLayout: SlipLayout;
}

const defaults: Settings = { theme: 'dark', slipLayout: '2x4' };

function load(): Settings {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem('vl.settings') ?? '{}') };
  } catch {
    return defaults;
  }
}

export const settings = $state<Settings>(load());

export function saveSettings() {
  try {
    localStorage.setItem('vl.settings', JSON.stringify(settings));
  } catch {}
}

export function applyTheme() {
  document.documentElement.dataset.theme = settings.theme;
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', settings.theme === 'dark' ? '#0a1020' : '#f1f4f9');
}

export function toggleTheme() {
  settings.theme = settings.theme === 'dark' ? 'light' : 'dark';
  saveSettings();
  applyTheme();
}
