# casino-backend

Actix Web service behind two frontends: a Phaser casino for players at `/`, and
an operator status dashboard at `/admin`.

## Quick start

```bash
cp .env.example .env
# put a real secret in JWT_SECRET, e.g.
openssl rand -hex 32

cd frontend && npm install && npm run build && cd ..
cargo run                                    # http://127.0.0.1:8080
```

Open the root URL, create an account, and you land in the lobby with 1,000
coins. Operator accounts are separate from player accounts, so bootstrap the
first one from the command line and then sign in at `/admin`:

```bash
cargo run --bin create-operator -- <username>              # admin by default
cargo run --bin create-operator -- <username> --role viewer

cargo run --bin create-user -- <username>   # a player, for testing
```

Without a `JWT_SECRET` the server generates a random one at startup and logs a
warning; tokens then stop working after every restart.

## How auth works

`POST /api/register` and `POST /api/login` verify or store an Argon2id hash and
return a signed JWT (HS256). The browser keeps it and sends it as
`Authorization: Bearer <token>` on every data request. Registration signs the
new player in directly, so the client does not need a second round trip.

**Players and operators are separate sets of accounts.** Anyone can register as
a player, so player credentials must never open the dashboard. Operators live in
their own `operators` table and sign in at `POST /api/admin/login`. Both token
kinds are signed with the same key, so each carries a `kind` claim (`player` or
`operator`) and every extractor demands the audience it expects: a player token
on `/api/status` is a 401, and so is an operator token on `/api/slots/spin`.

Usernames are unique case-insensitively, 3–24 characters of letters, numbers,
underscores, and hyphens. Passwords must be at least 10 characters. Both rules
live in `src/auth/policy.rs` and are enforced for the CLI too.

Logins for unknown usernames still run a full Argon2 verification against a
decoy hash, so response time does not reveal which accounts exist.

Because the token lives in `localStorage`, any successful XSS can read it. That
is the standard trade-off for browser-held JWTs; if this faces the public
internet with real money attached, move the token into an `HttpOnly` cookie.

## Endpoints

Player surface:

| Method | Path                  | Auth   | Purpose                                    |
| ------ | --------------------- | ------ | ------------------------------------------ |
| `GET`  | `/api/health`         | public | Liveness probe for load balancers          |
| `POST` | `/api/register`       | public | Create an account, staked with 1,000 coins |
| `POST` | `/api/login`          | public | Exchange credentials for a player token    |
| `GET`  | `/api/me`             | player | The current account, including balance     |
| `GET`  | `/api/balance`        | player | The current player's coin balance          |
| `GET`  | `/api/bank`           | player | Posted loan terms and the open marker, if any |
| `POST` | `/api/bank/borrow`    | player | Take a house loan; interest is charged now |
| `POST` | `/api/bank/repay`     | player | Clear the outstanding marker in full       |
| `GET`  | `/api/slots/paytable` | public | Symbols, payouts, stake limits, and RTP    |
| `POST` | `/api/slots/spin`     | player | Play one spin and settle the stake         |

Operator surface:

| Method   | Path                                | Auth     | Purpose                              |
| -------- | ----------------------------------- | -------- | ------------------------------------ |
| `POST`   | `/api/admin/login`                  | public   | Exchange credentials for an operator token |
| `GET`    | `/api/admin/me`                     | operator | The signed-in operator and their role |
| `PUT`    | `/api/admin/me/password`            | operator | Change your own password             |
| `GET`    | `/api/status`                       | operator | Status payload for the dashboard     |
| `GET`    | `/api/admin/operators`              | operator | List operator accounts               |
| `POST`   | `/api/admin/operators`              | admin    | Add an operator                      |
| `PATCH`  | `/api/admin/operators/{id}`         | admin    | Change role or active state          |
| `PUT`    | `/api/admin/operators/{id}/password`| admin    | Reset another operator's password    |
| `DELETE` | `/api/admin/operators/{id}`         | admin    | Delete an operator                   |
| `GET`    | `/api/admin/players`                | operator | List players (`search`, `limit`, `offset`) |
| `PATCH`  | `/api/admin/players/{id}`           | admin    | Ban or unban a player                |
| `POST`   | `/api/admin/players/{id}/balance`   | admin    | Credit or debit a player's coins     |
| `GET`    | `/api/admin/adjustments`            | operator | Audit log of balance adjustments (`player_id`, `limit`, `offset`) |

Pages:

| Method | Path           | Auth   | Purpose                                   |
| ------ | -------------- | ------ | ----------------------------------------- |
| `GET`  | `/`            | public | Phaser casino client                      |
| `GET`  | `/admin`       | public | Operator login page                       |
| `GET`  | `/status`      | public | Dashboard shell (data requires a token)   |
| `GET`  | `/admin/users` | public | Account management shell (same)           |

## Layout

```
src/
  config.rs      environment-driven configuration
  error.rs       ApiError -> JSON error responses
  state.rs       shared AppState (store, token service, start time)
  auth/          Argon2 hashing, credential policy, JWT, bearer extractor
  db/            Store trait + SQLite implementation
  models/        domain types, including STARTING_BALANCE, Loan, and Operator
  games/         slot machine math and house-bank terms
  routes/        /api handlers, /api/admin handlers, and the page routes
  bin/create_user.rs      creates a player
  bin/create_operator.rs  creates a dashboard login
migrations/      sqlx migrations, applied on startup
frontend/        Phaser client source (Vite); builds into static/
static/
  index.html     built Phaser client, served at /
  assets/        built JS and CSS (generated — edit frontend/ instead)
  admin/         hand-written operator login, status, and account pages
```

## The player frontend

`frontend/` is a Vite project that builds **into `static/`**, which the server
already serves at `/`. The build output (`static/index.html` and
`static/assets/`) is committed so a fresh clone runs with `cargo run` alone.

```bash
cd frontend
npm run dev      # http://localhost:5173, proxies /api to the Rust server
npm run build    # writes into ../static
```

Rebuild and commit `static/` whenever you change anything under `frontend/src`.

Scenes live in `frontend/src/scenes`: `BootScene` generates the artwork and
resumes a stored session, `AuthScene` handles sign-in and registration,
`LobbyScene` shows the balance, `BankScene` is the cashier, and `SlotsScene`
is the slot machine. All the graphics are drawn procedurally at boot
(`frontend/src/ui/textures.js`), so there are no image assets to serve.

When making a Container clickable, note that Phaser adds `displayOrigin` to the
local point before testing it, so **hit areas are measured from the container's
top-left, not from its centre**: use `Rectangle(0, 0, w, h)` or
`Circle(r, r, r)`. A shape centred on `(0, 0)` leaves the clickable region
offset up and to the left of the artwork, which still passes a test that only
ever clicks dead centre.

The sign-in form uses real `<input>` elements on Phaser's DOM layer rather than
canvas-drawn text, which keeps password managers, autofill, and accessibility
working. Two consequences worth knowing before you move things around:

- That DOM layer covers the whole game, so it is set to `pointer-events: none`
  and the form opts back in. Without this, no canvas button is clickable.
- Phaser positions DOM elements using a cached height, so the auth card is
  anchored by its top edge (`originY = 0`) and repositioned whenever fields or
  errors change its height.

The stage is a fixed 1280x720 scaled to fit, so the layout is built for
landscape. Portrait phones get heavy letterboxing.

## The slot machine

Three reels share one weighted strip, so every position is an independent draw
and the return to player can be stated exactly rather than estimated. The
weights and payouts live in one table in `src/games/slots.rs`:

| Symbol | Chance per reel | Three of a kind | Exactly two |
| ------ | --------------- | --------------- | ----------- |
| Clubs | 30% | 4x | 1x |
| Diamonds | 25% | 6x | 1x |
| Hearts | 18% | 12x | 1x |
| Spades | 12% | 25x | 2x |
| Star | 9% | 60x | 2x |
| Seven | 6% | 150x | 3x |

That works out to a **95.1% return to player**, so the house edge is 4.9%.
`theoretical_rtp()` derives this by enumerating all 216 combinations, and a
test fails if a paytable edit pushes it outside 90–97%.

Two rules the implementation depends on:

- **The server decides every spin.** The client posts only a stake; the reels
  come back in the response, and the animation just reveals them.
- **The stake and winnings move in one statement.** `settle_spin` guards the
  debit inside the `UPDATE` itself (`WHERE ... AND balance >= ?`) rather than
  reading the balance and then writing it, so simultaneous spins cannot both
  pass an affordability check and overdraw the account.

Every settled spin is written to the `spins` table with the reels and the
resulting balance, so balances can be reconciled against the play history.

## The house bank

Players who run out of coins can visit the cashier for a marker. The house
writes one loan at a time: you walk out with the principal, and you owe
principal plus 20% interest in a single repayment. A second borrow is refused
until the first is cleared. The repayment debit is guarded inside the
`UPDATE` the same way a spin is, and two concurrent borrows race the unique
index on an unpaid loan rather than a read-then-write check.

The posted amounts live in `src/games/bank.rs` (`250`, `500`, `1,000`,
`2,500`). The client only chooses from that list; anything else is refused.

## Operator accounts

Manage them at `/admin/users`, or through the endpoints above. Every operator
can read the dashboard and change their own password; `admin` operators can also
add, edit, and remove accounts, ban players, and adjust player balances.
`viewer` is the default for accounts created through the API, while the CLI
defaults to `admin` because it is how the first operator gets created.

Balance adjustments require a non-zero amount and a reason. The debit is
guarded inside the `UPDATE` so the balance cannot go below zero, and the same
transaction writes an audit row that records the operator, the signed delta,
the balances before and after, and the reason. Operator usernames are
snapshotted on that row so the log stays readable after the account is deleted.

Four rules make the permissions hold up in practice:

- **You cannot deactivate, demote, or delete yourself.** Since every other
  destructive action targets somebody else, this alone guarantees at least one
  active admin always remains — there is no way to empty the admin set.
- **Your own password goes through `PUT /api/admin/me/password`,** which
  requires the current one. The admin reset endpoint refuses to target you, so
  a stolen token cannot lock the real owner out without knowing their password.
- **Deactivating an operator takes effect immediately.** The extractor loads the
  operator on every request, so an existing token stops working at once instead
  of lasting until it expires.
- **Changing a password signs that account's other sessions out.** Operator rows
  carry a `password_changed_at` stamp, and any token issued before it is
  refused. This is what makes a password reset an effective response to a
  suspected compromise.

Banning a player works the same way: `is_banned` is checked on both sign-in and
every authenticated request, so a ban immediately stops play on tokens that were
already issued.

## Swapping SQLite for something else

Handlers only ever see `db::Store`, so a new backend means:

1. add the sqlx driver feature in `Cargo.toml`,
2. add `src/db/postgres.rs` implementing `Store`,
3. pick the implementation in `open_store()` based on the `DATABASE_URL` scheme.

Nothing in `routes/` or `auth/` needs to change.

## Adding status panels

`GET /api/status` returns `checks`, a list of `{ name, health, detail }`. Append
to `collect_checks()` in `src/routes/status.rs` and the dashboard renders the new
entry with no frontend changes. Anything richer than a check (charts, tables of
live tables or wallets) needs new fields plus rendering in
`static/admin/status.js`.

## Development

```bash
cargo test
cargo clippy --all-targets
cargo fmt
```
