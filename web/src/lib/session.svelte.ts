import { api, ApiError, onConnectivity } from './api';
import type { Me } from './types';

export const session = $state<{ me: Me | null; loading: boolean; online: boolean; offlineKind: 'network' | 'portal' | null }>({
  me: null,
  loading: true,
  online: navigator.onLine,
  offlineKind: null,
});

onConnectivity((online, kind) => {
  session.online = online;
  session.offlineKind = online ? null : (kind ?? 'network');
});
addEventListener('online', () => (session.online = true));
addEventListener('offline', () => {
  session.online = false;
  session.offlineKind = 'network';
});

const ME_KEY = 'vl.me';

export async function loadMe() {
  // Show the last known user at once; correct it when the server answers.
  try {
    const cached = localStorage.getItem(ME_KEY);
    if (cached) session.me = JSON.parse(cached);
  } catch {}
  try {
    setMe(await api.me());
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) setMe(null);
  } finally {
    session.loading = false;
  }
}

export function setMe(me: Me | null) {
  session.me = me;
  try {
    if (me) localStorage.setItem(ME_KEY, JSON.stringify(me));
    else localStorage.removeItem(ME_KEY);
  } catch {}
}

export async function logout() {
  try {
    await api.logout();
  } finally {
    setMe(null);
  }
}
