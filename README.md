# vttl-league

Match sheet for the home matches of one club (default `WVL154`) on a league Saturday:
lineups, individual results, ESP32 scoreboards over MQTT, and a TabT upload line for VTTL.

```
cp .env.example .env   # fill in MQTT and (optionally) VTTL credentials
cargo run              # http://127.0.0.1:3100
cargo test
```

## Architecture

- `src/vttl.rs`: SOAP client (GetMatches, GetMembers, GetMatchSystems, GetSeasons, Upload), 60 s cache.
- `src/mqtt.rs`: subscribes to `tabletennis/<id>/#` for each added device, decodes the state (`src/scoreboard.rs`), sends names.
- `src/db.rs`: SQLite. Lineups and results carry a `rev`; a write with an old rev gets HTTP 409.
- `src/upload.rs`: builds the I-line. Positions without a finished local result keep the VTTL result.
- `src/api.rs`: HTTP API plus `/ws`, which pushes every change to all browsers.
- `static/`: plain HTML + JS.

## Rules

- `DRY_RUN=true` (default) only shows the I-line. There is no VTTL test database.
- A manual result always wins over a scoreboard result. Clear the manual score to let the scoreboard write again.
- Linking a scoreboard sends the names (home player = player 1). The first device match that starts after the
  link belongs to that position. A finished match ends the link.
- The app only listens to scoreboards in the device list (the `device` table). Add a device with its ID and
  an alias; the app then subscribes to `tabletennis/<id>/#`. Removing a device unsubscribes and ends its link.
- Doubles are not supported yet (no match system of the club uses them today).
