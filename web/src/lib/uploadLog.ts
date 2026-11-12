// The VTTL log of a match: one row per upload attempt of the server (src/uploader.rs).

import type { UploadLogEntry } from './api';
import { keyLabel } from './sheet/vttl';

export type LogStatus = 'synced' | 'pending' | 'error';

/** Status and short text of one attempt. `detail` = the long text, for a problem only. */
export function logResult(e: UploadLogEntry): { status: LogStatus; text: string; detail: string } {
  if (e.dryRun) return { status: 'pending', text: 'proef', detail: '' };
  if (e.confirmed === true) return { status: 'synced', text: 'bevestigd', detail: '' };
  if (e.confirmed === false) return { status: 'error', text: 'VTTL toont andere gegevens', detail: e.diff.map(keyLabel).join(', ') };
  if (!e.ok) return { status: 'error', text: 'mislukt', detail: e.errors.join('; ') || 'onbekende fout' };
  return { status: 'pending', text: 'niet gecontroleerd', detail: e.checkError ?? '' };
}

export const SOURCE_TEXT: Record<string, string> = { auto: 'na wijziging', manual: 'uploadknop', retry: 'nieuwe poging' };
