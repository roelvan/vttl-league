// Per-device preferences (like the ScoreSheet settings window), kept in local storage.

interface Settings {
  /** "Officiële velden onveranderbaar maken" */
  protectMatchInfo: boolean;
  /** Show the team score on the navigation tiles. */
  showResultsInNav: boolean;
  /** Also open away matches for a new match day. */
  openAway: boolean;
  /** Also open byes of that week. */
  openByes: boolean;
  zoom: number;
  /** Live partial uploads to VTTL while the match is played (ScoreSheet: on). */
  liveUpdates: 'on' | 'super' | 'off';
}

const defaults: Settings = { protectMatchInfo: true, showResultsInNav: true, openAway: false, openByes: false, zoom: 1, liveUpdates: 'on' };

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
