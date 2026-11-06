// Shapes of the server API (camelCase JSON from src/api.rs and src/tabt.rs).

export interface Account {
  account: string;
  member: number | null;
  memberName: string | null;
  club: string | null;
}

export interface Venue {
  name: string;
  street: string;
  town: string;
  phone: string;
  comment: string;
}

export interface Club {
  id: string;
  name: string;
  longName: string;
  category: number;
  categoryName: string;
  venues: Venue[];
}

export interface Me {
  account: Account;
  club: Club | null;
  currentSeason: number | null;
}

export interface Member {
  position: number;
  uniqueIndex: number;
  rankingIndex: number;
  firstName: string;
  lastName: string;
  ranking: string;
  status: string;
  club: string;
  gender: string;
  category: string;
}

export interface Season {
  id: number;
  name: string;
  isCurrent: boolean;
}

export interface Team {
  teamId: string;
  letter: string;
  divisionId: number;
  divisionName: string;
  divisionCategory: number;
  matchType: number;
}

export interface ClubInfo {
  club: Club | null;
  teams: Team[];
  season: number;
}

export interface MatchRow {
  matchId: string;
  uniqueId: number;
  week: string;
  date: string;
  time: string;
  venue: Venue | null;
  homeClub: string;
  homeTeam: string;
  awayClub: string;
  awayTeam: string;
  score: string;
  divisionId: number;
  divisionCategory: number;
  isHomeForfeited: boolean;
  isAwayForfeited: boolean;
  isValidated: boolean;
  isLocked: boolean;
  isBye: boolean;
  isHome: boolean;
  teamLetter: string | null;
  divisionName: string | null;
  matchType: number | null;
  matchSystem: number | null;
  divisionLevel: number | null;
}

export interface MatchList {
  season: number;
  teams: Team[];
  matches: MatchRow[];
}

export interface Player {
  position: number;
  uniqueIndex: number;
  firstName: string;
  lastName: string;
  ranking: string;
  victoryCount: number;
  isForfeited: boolean;
}

export interface DoubleTeam {
  position: number;
  team: number;
  isForfeited: boolean;
}

export interface IndividualResult {
  position: number;
  homeIndexes: number[];
  awayIndexes: number[];
  homeSets: number;
  awaySets: number;
  isHomeForfeited: boolean;
  isAwayForfeited: boolean;
  scores: string;
}

export interface MatchDetails {
  detailsCreated: boolean;
  matchSystem: number;
  homeCaptain: number;
  awayCaptain: number;
  referee: number;
  homePlayers: Player[];
  awayPlayers: Player[];
  homeDoubles: DoubleTeam[];
  awayDoubles: DoubleTeam[];
  results: IndividualResult[];
  homeScore: number;
  awayScore: number;
}

export interface TeamMatch extends Omit<MatchRow, 'isHome' | 'teamLetter' | 'divisionName' | 'matchType' | 'matchSystem' | 'divisionLevel'> {
  venueClub: string;
  isHomeWithdrawn: boolean;
  isAwayWithdrawn: boolean;
  details: MatchDetails | null;
}

export interface PlayerCategory {
  id: number;
  name: string;
  shortName: string;
  rankingCategory: number;
  sex: string;
  minAge: number | null;
  maxAge: number | null;
  isGroup: boolean;
}

export interface MatchDef {
  position: number;
  kind: number;
  homeIndex: number;
  awayIndex: number;
  allowSubstitute: boolean;
}

export interface MatchSystem {
  id: number;
  name: string;
  setCount: number;
  pointCount: number;
  singleMatchCount: number;
  doubleMatchCount: number;
  forcedDoubleTeams: boolean;
  substituteCount: number;
  defs: MatchDef[];
}
