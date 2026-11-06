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

  /** Set after the first full upload; live partial uploads stop then. */
  disablePartialUpload?: boolean;
}

export const emptyPlayer = (): SheetPlayer => ({ name: '', computerNumber: '', position: '', index: '', ranking: '', captain: false });
export const emptyPerson = (): Person => ({ name: '', computerNumber: '', clubName: '', clubId: '' });
