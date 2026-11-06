# vttl-league

Een webappkloon van het programma [ScoreSheet](https://score.pieterp.be/) van PieterP.be:
the Windows match-sheet program for the Belgian table tennis competition, rebuilt as a web app
for VTTL clubs. Users log in with their own VTTL account.

The ScoreSheet rules (match systems, scores, validations) are ported from its source code
(© 2024 PieterP.be, BSD licence, see `NOTICE-ScoreSheet.md`). Thanks to the ScoreSheet team.
This app is not an official product of PieterP.be or the VTTL.

```
cp .env.example .env
cargo run                      # API on http://127.0.0.1:3100, serves web/dist
cd web && pnpm install
pnpm dev                       # client with hot reload on http://127.0.0.1:5173 (proxies /api)
pnpm build                     # production client in web/dist
cargo test && pnpm check
```

## Architecture

- `src/tabt.rs`: VTTL TabT SOAP client. One shared cache for all users (the key has no
  credentials); concurrent requests for the same data share one HTTP call; an expired entry is
  served at once while it refreshes in the background.
- `src/auth.rs`, `src/crypto.rs`: login with `Test` (`IsValidAccount`). The VTTL password is
  kept encrypted (ChaCha20-Poly1305) in the session row, because uploads need it. The browser
  only holds a random token in an HttpOnly cookie; the database keeps its SHA-256.
- `src/db.rs`: SQLite (WAL). Migrations are numbered with `PRAGMA user_version`.
- `src/api.rs`: JSON API. Errors are `{ "error": code, "message": text }`.
- `web/`: Svelte 5 + Vite client. It keeps the last data in local storage and shows it at once.
  A redirect or a non-JSON answer counts as "offline": the hall WiFi has a captive portal.
- `src/sheets.rs`: match sheets. One JSON document per (club, sheet), changed with "set"
  operations (last write wins per field). A batch carries a client id + sequence number, so a
  retry after a lost answer is not applied twice. `/api/ws` pushes every change to the club.
- `web/src/lib/sheet/`: the ScoreSheet rules in TypeScript (match systems, score, validations,
  new sheet from VTTL), so they also work offline. Tests: `pnpm test`.
- `web/src/lib/workspace.svelte.ts`: open sheets, the queue of unsent changes (kept in local
  storage), sync and the WebSocket.
- `web/public/sw.js`: service worker. It keeps the app on the device; a page without the
  `x-vl-app` header (a captive-portal page) is replaced by the cached app.
- `src/scoreboard.rs`: decoder for the ESP32 scoreboard (not linked yet).

## Differences with ScoreSheet (on purpose)

- An unfinished game (sets entered, not decided) counts as incomplete; ScoreSheet marks the next
  empty box as an error while you type.
- The optional Super substitute (V) may stay empty without a validation error.

## VTTL API facts (checked 2026-11-06)

- `Test` gives no club and no rights for an account. A user picks their own name once
  (`GetMembers` `NameSearch`); the club of that member becomes the club of the account.
- A wrong login gives `IsValidAccount=false` with HTTP 200, not a fault.
- `MatchUniqueId` only exists for played matches. `MatchId` (e.g. `PWVLH07/012`) is the key.
- A match between two teams of the same club comes twice in `GetMatches` for that club.
- `GetMatches` with `Club` + `Season` + `WithDetails` returns the whole season of a club in
  one call (~1.5 s, ~650 kB XML).
- There is no VTTL test database: uploads stay in dry-run mode by default.
