# vttl-league

Interclub Live: follow and score VTTL team matches live, per club night, with an optional link
to Flippr/ESP32 scoreboards. Club members log in with their own VTTL account; anyone can follow a
club night or a match without a login on a public link (`/c/{club}/…`).

The match rules (match systems, scores, validations, upload lines) are ported from
[ScoreSheet](https://score.pieterp.be/) of PieterP.be.

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
- `web/src/lib/workspace.svelte.ts`: the stored matches, sync and the WebSocket. The server is
  the truth: every change goes to it at once. Local storage only holds the queue of changes that
  wait for a connection. A match without a stored record is a local draft from VTTL until the
  first edit. The UI never shows the word "sheet".
- `web/src/lib/live.ts`, `night.ts`: view model of the live screens (games, tables in fixed
  pairs, race bar, tallies) for every match system.
- `web/public/sw.js`: service worker. It keeps the app on the device; a page without the
  `x-vl-app` header (a captive-portal page) is replaced by the cached app.
- `src/uploader.rs`, `src/rules.rs`: the server uploads to VTTL by itself, a few seconds after
  every stored change: a partial upload when a game is decided, the full upload when the result
  is complete. The upload lines are built on the server (`rules.rs`, a port of `export.ts`,
  `score.ts` and `systems.ts`). The web tests write `tests/upload-fixtures.json` and the Rust test
  checks the same output. Login: the account of the last change, then other club accounts
  (accounts with a real upload first). Dry run unless `UPLOAD_ENABLED=true`; every upload is
  logged in the `upload` table and written into the match (`upload`, `vttlSeen`).
  `POST /api/upload` = manual retry with the user's own login first.
- `web/src/lib/sheet/vttl.ts`: two-way sync. Sheet and VTTL are compared as one key -> text map.
  A VTTL value wins when it is new or changed since the last import (`vttlSeen`); an unchanged
  or empty VTTL value never overwrites a local edit. Import on open and every 5 minutes for
  matches of today/yesterday.
  Validated/locked on VTTL = read-only. The status icon (✓ ↑ ! 🔒) compares sheet and `vttlSeen`.
- `web/src/components/print/`: the VTTL scheidsrechtersblad (A4 landscape, positions in cm from
  ScoreSheet's VttlScoreTemplate.xaml) and the referee slips (2x4, 1x8, per table). Printing uses
  the browser; "Opslaan als PDF" gives the PDF. Print class names have a `ps-`/`rs-` prefix so
  the screen CSS cannot leak in.
- `src/boards.rs`, `src/scoreboard.rs`: scoreboards over MQTT (`MQTT_URL`, `MQTT_USER`,
  `MQTT_PASSWORD`; off when empty). Per club: device id + alias. "Naar scorebord" sends the
  names (`tabletennis/{id}/config`) and links the board to one game; updates of unlinked boards
  or another board match are ignored. A finished board match writes its set scores.
- `src/public.rs`: the read-only public API and WebSocket (no login, no account names).

## Deploy (https://competitie.ttczonnebeke.be)

Server: `westtoer-directus` (Hetzner cax11, ARM64, Ubuntu 24.04), SSH only via Tailscale. It also
runs nginx, mosquitto and Directus. No Docker: systemd runs the binary on 127.0.0.1:3100, nginx
(certbot TLS) proxies to it. DNS: A record `competitie` in Cloudflare, DNS only (grey cloud).

```sh
sh deploy/deploy.sh          # build web/ here, build the binary on the server, restart
sh deploy/setup.sh [--tls]   # one-time server setup (safe to run again)
```

- Binary + web files: `/opt/vttl-league` (previous release kept as `*.prev`, `REVISION` = commit).
- Data: `/var/lib/vttl-league/vttl-league.sqlite`. Settings: `/etc/vttl-league.env` (secret key
  made on the server). `UPLOAD_ENABLED=false` until the first real upload is tested.
- Logs: `journalctl -u vttl-league -f`.

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
