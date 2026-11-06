import type { Sheet } from './sheet/types';
import { getSystem } from './sheet/systems';
import { validate } from './sheet/validate';
import { members, clubExists, defaultCategory } from './data.svelte';
import { ws } from './workspace.svelte';

/** Runs the ScoreSheet validations with the data this device has. */
export function validateSheet(s: Sheet): string[] {
  return validate(s, getSystem(s.matchSystemId), {
    today: new Date(),
    members,
    clubExists,
    defaultCategory: defaultCategory(),
    sameDay: Object.values(ws.entries)
      .map((e) => e.view)
      .filter((o) => o.day === s.day),
    systemOf: getSystem,
  });
}
