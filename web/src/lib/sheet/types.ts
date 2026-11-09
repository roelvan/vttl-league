// The match sheet document. Same content as the ScoreSheet `Match` entity
// (PieterP.ScoreSheet.Model/Database/Entities/Match.cs), so the export and upload work the same.
//
// Every field is a plain value, so a change is always "set this path to this value"
// (see ops.ts). Do not store derived values (scores, wins) in the document.

export interface SetScore {
  l: string;
  r: string;
}

/** One single-player slot. All values are text, as typed on the sheet. */
export interface SheetPlayer {
  name: string;
  computerNumber: string;
  /** Strength-list position ("volgnummer"). */
  position: string;
  index: string;
  ranking: string;
  captain: boolean;
}

export interface Person {
  name: string;
  computerNumber: string;
  clubName: string;
  clubId: string;
}

export interface SheetTeam {
  name: string;
  clubId: string;
  forfeit: boolean;
  /** One entry per single-player slot of the match system, in slot order. */
  players: SheetPlayer[];
  /** One entry per double slot: "1/2", "1/3", … */
  doubles: string[];
  /** One entry per substitute slot: the game position where the substitute plays, or null. */
  substitutes: (number | null)[];
}

export type Level = 'super' | 'national' | 'regional' | 'provincial';

/** What VTTL said when the sheet was made. The validations compare against it. */
export interface OfficialInfo {
  matchId: string;
  /** dd/mm/yyyy, "" for a bye without a date */
  date: string;
  series: string | null;
  homeTeam: string;
  awayTeam: string;
}

export interface Sheet {
  v: 1;
  id: string;
  season: number;
  /** ISO date the sheet belongs to (for the match-day list). */
  day: string;
  isOfficial: boolean;
  matchSystemId: number;
  playerCategory: number;
  official: OfficialInfo | null;

  address: string;
  /** dd/mm/yyyy, as on the paper sheet */
  date: string;
  startHour: string;
  endHour: string;
  matchId: string;
  series: string;
  level: Level;
  men: boolean;
  women: boolean;
  interclub: boolean;
  super: boolean;
  cup: boolean;
  youth: boolean;
  veterans: boolean;
  article632: boolean;
  comments: string;

  home: SheetTeam;
  away: SheetTeam;
  /** results[game][set] */
  results: SetScore[][];

  chiefReferee: Person;
  homeCaptain: Person;
  awayCaptain: Person;
  roomCommissioner: Person;

  /** Live follow: the line-up is done and the evening has started ("Start de avond"). */
  started?: boolean;
  /** The tables in play, in order. Games go out in fixed pairs: game i is on table i % length. */
  tables?: TableSetup[];

  /** Set after the first full upload; live partial uploads stop then. */
  disablePartialUpload?: boolean;

  /** Last VTTL value seen per field (see vttl.ts `canonical`), for the two-way sync. */
  vttlSeen?: Record<string, string>;
  /** VTTL validated or locked the result: the sheet is read-only. */
  vttlLocked?: boolean;
  /** The last upload (or dry run) of this sheet. */
  upload?: UploadInfo;
}

export interface TableSetup {
  /** Table number in the hall, as typed. */
  no: string;
  /** Scoreboard device id on this table, or null for manual entry. */
  device: string | null;
}

export interface UploadInfo {
  at: number;
  by: string;
  ok: boolean;
  partial: boolean;
  dryRun: boolean;
  errors: string[];
}

export const emptyPlayer = (): SheetPlayer => ({ name: '', computerNumber: '', position: '', index: '', ranking: '', captain: false });
export const emptyPerson = (): Person => ({ name: '', computerNumber: '', clubName: '', clubId: '' });
