// A clock for countdowns and running durations. It ticks only while a component reads it,
// and once more when the page becomes visible again (a phone that slept).

import { createSubscriber } from 'svelte/reactivity';

const subscribe = createSubscriber((update) => {
  const id = setInterval(update, 250);
  const onvis = () => update();
  document.addEventListener('visibilitychange', onvis);
  return () => {
    clearInterval(id);
    document.removeEventListener('visibilitychange', onvis);
  };
});

/** Date.now(), reactive: reading it in a template or $derived re-runs it 4 times a second. */
export function now(): number {
  subscribe();
  return Date.now();
}
