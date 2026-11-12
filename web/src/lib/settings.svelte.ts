// Per-device preferences, kept in local storage.

interface Settings {
  /** Dark is the default (design); the sun button switches. */
  theme: 'dark' | 'light';
  /** Match order: two columns (game 1 | 2 side by side, as on table 1 | 2) or one column. */
  order: 'grid' | 'list';
}

const defaults: Settings = { theme: 'dark', order: 'grid' };

function load(): Settings {
  try {
    const { theme, order } = JSON.parse(localStorage.getItem('vl.settings') ?? '{}');
    return { ...defaults, ...(theme ? { theme } : {}), ...(order === 'grid' || order === 'list' ? { order } : {}) };
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
