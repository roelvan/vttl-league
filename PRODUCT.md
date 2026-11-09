# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Scorers (club members, logged in with their own VTTL account).** During a club night they
  set the line-up, enter game scores and follow every home match of the evening. Devices are
  mixed: captains use their phones next to the tables, and a shared tablet or laptop at the bar
  or referee table is also in use. Both must work equally well.
- **Followers (public, no login).** Club members and family follow a club night or one match
  from home on a phone, and a TV or large screen in the club canteen shows the club night live.
- Any club member may edit; an account gets a "can upload" mark after its first real upload.

## Product Purpose

Interclub Live lets a VTTL table tennis club follow and score its interclub team matches live,
per club night. It replaces the paper/ScoreSheet tab-and-sheet workflow with a live view of all
matches of the day, uploads results to VTTL as soon as possible (a VTTL requirement), and can
link Flippr/ESP32 scoreboards so table scores arrive by themselves.

Success: the scorers of a club night finish every match without paper, the result is on VTTL
without a manual step, and anyone can follow the evening live.

## Positioning

- Built around the **club night**, not the sheet: the screen is "the VTTL matches of this day",
  and the stored match record appears silently on the first edit. The UI never says "sheet".
- **Live scoreboard link:** a tap sends player names to a table scoreboard over MQTT and binds
  that board to one game; the finished board result is saved automatically.
- **Server-side upload to VTTL:** partial upload after each decided game, full upload at
  completion, with two-way sync against VTTL.
- **Public follow links** (`/c/{club}/…`) without an account.

## Operating Context

- Used in a sports hall during a club night: several matches run at once, two tables per match
  in fixed pairs (odd games table 1, even games table 2), noise, hurried input between games.
- The hall WiFi is unreliable and has a captive portal. The app must keep working offline: last
  data from local storage, a queue of changes, a service worker that keeps the app on the device.
  Fonts and assets are bundled; nothing may depend on a CDN at match time.
- Printing: the official VTTL scheidsrechtersblad (A4 landscape) and referee slips, via the
  browser print dialog or "Opslaan als PDF".
- Flow per match: line-up (players, officials, details) → games with scores → validation →
  upload. Line-up changes after game 1 are allowed with a warning.

## Capabilities and Constraints

- UI language: Dutch. Terminology follows VTTL usage (interclub, ploegopstelling,
  scheidsrechtersblad, W-O, dubbel).
- All VTTL match systems are supported generically, including doubles and W-O.
- Away matches on a club night show the VTTL score only.
- A match validated or locked on VTTL is read-only.
- Scoreboard devices are set per club (device id + alias); any editor manages them. The board is
  picked per table in the line-up.
- Uploads are dry-run unless the server enables them.
- Match rules are ported from ScoreSheet (PieterP.be, BSD licence). The app is not an official
  product of PieterP.be or the VTTL and must not present itself as one.
- Open for any VTTL club. No club is the default and the product identity stays club-neutral;
  the first deployment is TTC Zonnebeke (vttl.ttczonnebeke.be).
- Not in v1: confirmation by the away captain.

## Brand Commitments

- Name: **Interclub Live**.
- Credit to ScoreSheet / PieterP.be stays (README, `NOTICE-ScoreSheet.md`).
- No VTTL or club branding that suggests an official VTTL product.

## Evidence on Hand

- Real data comes live from the VTTL TabT API (clubs, teams, players, matches).
- No testimonials, usage numbers or club endorsements exist; do not invent them.

## Product Principles

1. **The match never waits for the app.** Input between games must be fast and must survive
   bad WiFi; offline is a normal state, not an error.
2. **One truth, shown everywhere.** Scorer, follower, hall screen and scoreboard show the same
   live state; conflicts with VTTL resolve by clear rules, never silently.
3. **Club night first.** People think in "tonight's matches", not in documents or files.
4. **Done means uploaded.** A finished match is on VTTL without a separate manual job.
5. **Neutral for every club.** Nothing ties the product to one club's identity.

## Accessibility & Inclusion

- Must be readable at a distance (canteen screen) and in a bright or dim sports hall.
- Touch targets fit hurried input on a phone held in one hand.
- No product-specific standard was set beyond this.
