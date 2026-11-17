# Naira Stash

A mobile-first savings app in naira (NGN). People save a fixed amount on a schedule, top up with extra deposits, and withdraw. Group savings circles (Esusu-style) are planned.

## How the money works

Naira Stash is **wallet-first**, the same model Bitnob uses.

1. **Deposit:** money goes from the bank into the user's wallet through Paystack.
2. **Fund:** savings plans take money out of the wallet (automatically on schedule, or by manual top-up). The wallet drops, the plan balance rises, and a ledger row is written.
3. **Withdraw:** money comes out of a plan.

All amounts are stored in **kobo** as `i64` (₦1 = 100 kobo) and shown as naira text.

**Catch-up rule (planned):** if scheduled saves are missed, the next deposit pays the missed days first, one full amount each. Anything left over that is less than one full amount stays in the wallet. Example: saving ₦2,000 daily, 5 days missed, then ₦11,000 arrives, so 5 days are covered (₦10,000) and ₦1,000 stays idle in the wallet.

**Approval model:** personal plan withdrawal is instant. Group withdrawal and group loans need the group admin. Rule changes are admin only. There is no member voting.

## Tech

Rust (edition 2024), axum 0.7, tokio, askama 0.12 server-rendered templates, htmx 2 (`hx-boost` on body), Tailwind via CDN, sqlx 0.8 on Postgres, argon2, sha2, hmac, reqwest, chrono, uuid (`v4` only, no serde feature), dotenvy.

## Architecture

Vertical slicing: one folder per feature, four files each.

| File | Job |
|---|---|
| `repo.rs` | database queries only |
| `services.rs` | business rules, no HTTP types |
| `handlers.rs` | HTTP in and out, calls the service |
| `routes.rs` | URL to handler wiring |

A feature has a root file next to its folder (`plans.rs` beside `plans/`), not a `mod.rs`. Shared helpers live in `src/shared/` (`money.rs` for naira and kobo, `render.rs` for rendering any template). Features never import from each other except through those helpers and the `AuthUser` guard.

## Run it

```bash
# .env needs DATABASE_URL, PAYSTACK_SECRET_KEY and APP_CALLBACK_URL
sqlx migrate run
cargo run
```

The app runs at http://127.0.0.1:3000. Paystack's webhook cannot reach a local address, so use a tunnel (such as ngrok) to test the wallet being credited.

## Status (17 Nov 2026)

### Working

- **Auth:** signup (user and wallet in one transaction), login (argon2), logout, cookie sessions (hashed token, HttpOnly, 7-day cap) and a **20-minute idle timeout** enforced in the database.
- **Deposits:** `POST /api/deposits/initialize` (minimum ₦100) creates a pending deposit and a Paystack checkout link. `POST /api/deposits/webhook` verifies the signature and credits the wallet once, in a transaction.
- **Dashboard:** real wallet balance. The deposit page shows the wallet balance.
- **Plans:** create (name, daily, weekly or monthly, amount, flexible or locked with end date, optional target), list with real data and search, and a real detail page.

### Not built yet

- **Funding a plan from the wallet** and the transaction ledger (next feature). Plan balances stay at ₦0 until then.
- **Passbook streak ledger** on the plan detail page (empty state for now).
- **Automatic scheduled debit** and the missed-day catch-up above.
- **Withdrawals and loans:** mock pages only, and the `withdrawals` and `transaction_history` tables have no code yet.
- **Groups:** mock pages only.
- **Activity, notifications and linked bank accounts:** mock pages only.

### Known rough edges

- No "deposit received" page after paying on Paystack.
- `dashboard.html` still shows "No plans yet" and "Auto-debit active" as fixed text.
- The plan form does not refill what the user typed after an error.
- `profile.rs` keeps repo, service and handler in one file.
- `auth.rs` has its own `render` function that only accepts `LoginTemplate`.
- `LoanReviewTemplate` has a field named `Security` (style warning).

## Roadmap

1. Database: done
2. Auth and sessions: done
3. Plans pages: done. Next: funding, ledger, activity page, new group flow
4. Admin side
5. Payments: Paystack deposits done. Bank transfer and withdrawals to do
6. Split templates into htmx fragments
