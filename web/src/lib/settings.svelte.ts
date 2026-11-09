// Per-device preferences, kept in local storage.

interface Settings {
  /** Dark is the default (design); the sun button switches. */
  theme: 'dark' | 'light';
}

const defaults: Settings = { theme: 'dark' };

function load(): Settings {
  try {
    const { theme } = JSON.parse(localStorage.getItem('vl.settings') ?? '{}');
    return { ...defaults, ...(theme ? { theme } : {}) };
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
