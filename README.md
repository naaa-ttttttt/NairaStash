# Naira Stash

A mobile-first savings app in naira (NGN). People save a fixed amount on a schedule, add money to a plan from their wallet, and withdraw. It is built first for a real thrift cooperative (collectors tick customers' cards by hand), and also works for individuals. Group savings circles (Esusu or ajo style) are planned.

## Features

- **Accounts:** signup, login, logout, cookie sessions with a 20-minute idle timeout.
- **Wallet:** fund it from a bank card or transfer through Paystack.
- **Savings plans:** a daily, weekly or monthly plan with a required end date and a target.
- **Add money:** tap **Add money** on a plan, a sheet slides up, type an amount, and whole periods move from the wallet into the plan.
- **Activity:** every money movement is a row in an append-only ledger, with a detail page for each one.
- **Themes:** a dark (dark green) and a light theme. The app follows the phone setting, and a round sun/moon button in every header switches and remembers the choice.

## How the money works

Naira Stash is **wallet-first**, the same model Bitnob uses.

| Step | Moves | Ledger type |
|---|---|---|
| Deposit | bank to wallet (Paystack) | `deposit` |
| Add money | wallet to plan | `plan_contribution` |
| Plan withdrawal | plan to wallet | `plan_withdrawal` |
| Payout to bank (later) | wallet to bank | `payout` |

All amounts are stored in **kobo** as `i64` (₦1 = 100 kobo) and shown as naira text.

### Plan rules

- Every plan has a **required end date** and a **target**.
- With a target, the app works out how much to save each period (rounded **up** to a whole naira). Without one, you type the amount per period. Either way the stored target is `amount x number of periods`.
- The number of periods counts the start day. The last period may fall **on** the end date. Example: a daily plan from Jan 1 to Jan 10 has 10 periods.
- **Add money moves whole periods only.** The leftover stays in the wallet. Less than one period is refused. It is also refused when the wallet is too low, the plan is closed, or every period is already filled.
- Each period added is its own ledger row, and every row records the wallet balance right after it.
- **Locked** plans allow no withdrawal before the end date. **Flexible** plans allow early withdrawal with a 3% fee, taken on top (planned, see Status).
- A plan ends in one of three ways: **liquidation** (the whole balance taken out early), **target reached**, or **end date**.

### Planned rules

- **Catch-up:** if scheduled saves are missed, any new money fills the earliest empty periods first, one full amount each. Anything under one full amount stays in the wallet.
- **Approval model:** a personal plan withdrawal is instant. Group withdrawals and loans need the group admin. Rule changes are admin only. There is no member voting.

## Tech

Rust (edition 2024), axum 0.7, tokio, askama 0.12 server-rendered templates, htmx 2 (`hx-boost` on body), Tailwind via CDN, sqlx 0.8 on Postgres, argon2, sha2, hmac, reqwest, chrono, uuid (`v4` only, no serde feature), dotenvy.

## Architecture

Vertical slicing: one folder per feature, four layers each.

| File | Job |
|---|---|
| `repo.rs` | database queries only |
| `services.rs` | business rules, no HTTP types, opens the database transaction |
| `handlers.rs` | HTTP in and out, calls one service |
| `routes.rs` | URL to handler wiring |

A feature has a root file next to its folder (`plans.rs` beside `plans/`), not a `mod.rs`. Every feature is also listed in `src/domains.rs`. Shared helpers live in `src/shared/` (`money.rs` for naira and kobo, `render.rs` for rendering any template).

Features do not import each other, with a few deliberate exceptions: `plan_funds` writes through the `ledger` service, and the dashboard reads from `plans` and `ledger`. Every feature has its own error enum, and handlers turn errors into messages.

Money-moving code runs in **one database transaction**: lock the plan row (`SELECT ... FOR UPDATE`), take money from the wallet with a guarded `UPDATE ... WHERE balance >= amount`, credit the plan, write the ledger rows, commit. If any step fails, nothing is kept.

```text
src/
  main.rs            router and app state
  domains.rs         list of features
  domains/
    auth/  dashboard/  deposits/  ledger/  plans/  plan_funds/
    groups/  withdrawals/  profile/
  shared/            money.rs, render.rs
templates/           askama html templates
migrations/          sqlx migrations
```

## Design

The look is a mature fintech app: dark green and white, with gold as the third colour. A reference fintech app (balance hero card, quick-action row, plain list rows, a tab bar with a pill on the active tab) set the direction.

**Theme roles.** Colours are defined once in `templates/base.html` as CSS variables, with one set of values for dark and one for light. Pages use only role names, never hex values.

| Role | Use |
|---|---|
| `page`, `surface`, `sunken` | backgrounds: the page, cards, and blocks inside cards |
| `line` | 1px borders and progress tracks |
| `ink`, `muted` | main and quiet text |
| `accent`, `on-accent` | the primary button, selected chips, the active tab |
| `mint` | progress, links, credits, "active" and "approved" states |
| `gold` | pending, behind and "required" states, plus the few brand spots on the hero card |
| `danger` | declined, failed and error states |
| `hero` | the balance card gradient |

**Shared classes** (also in `base.html`): `app-header`, `card`, `card-inset`, `btn-primary`, `btn-quiet`, `hero-card`, `tab-on`, `tab-off`.

**Rules for every page:**

- Solid surfaces with a 1px border. No transparency, no blur, no shadows.
- One gradient only: `.hero-card` (the dashboard balance card and the login header). Its text is always white.
- Corners: cards and buttons 16px, inputs 12px. Full pills only for the active tab, tab switches, filter chips, status chips and round icon buttons.
- `btn-primary` is for the main action only. Other buttons use `btn-quiet`.
- Pages that hide the tab bar use a fixed solid bottom dock (`bg-page border-t border-line`). Pages with the tab bar end in a normal `btn-primary`. There are no floating buttons.
- Long naira amounts must not be cut off: `break-all` or a smaller font.
- Tailwind runs from the CDN with its config inside `base.html`, so classes written inside JavaScript strings still work.

## Run it

You need Rust, Postgres and `sqlx-cli` (`cargo install sqlx-cli`).

```bash
# .env needs DATABASE_URL, PAYSTACK_SECRET_KEY and APP_CALLBACK_URL
sqlx migrate run
cargo run
```

The app runs at http://127.0.0.1:3000.

Paystack's webhook (`POST /api/deposits/webhook`) cannot reach a local address, so use a tunnel such as ngrok to test the wallet being credited. The ngrok address changes on every restart, and the webhook URL in the Paystack dashboard must match it.

To create a migration: `sqlx migrate add <name>`, edit the new file, then `sqlx migrate run`.

## Status (21 Nov 2026)

### Working

- **Auth:** signup (user and wallet in one transaction), login (argon2), logout, cookie sessions (hashed token, HttpOnly, 7-day cap) and a 20-minute idle timeout enforced in the database. The signup form has a show/hide button on both password fields.
- **Deposits:** `POST /api/deposits/initialize` (minimum ₦100) creates a pending deposit and a Paystack checkout link. The webhook verifies the signature and, in one transaction, credits the wallet and writes a ledger row.
- **Ledger and activity:** an append-only `ledger_entries` table (a trigger blocks updates and deletes) with a `fee` column. The `/activity` and `/activity/:entry_id` pages show real transactions.
- **Plans:** create a plan with a required end date and a target. The form asks for the end date first, then an optional target, and refills what you typed after an error. List, search and detail pages use real data.
- **Add money:** `POST /plans/:plan_id/add`. On the plan page, the **Add money** button opens a bottom sheet with quick buttons (1, 2 or 5 periods) and a live preview of what moves and what stays in the wallet.
- **Dashboard:** real wallet balance with a hide-balance eye, four quick actions, the latest 3 active plans, and the latest 5 ledger rows.
- **Deposits chart:** "Deposits this week" on the dashboard shows 7 bars, one per day, with the naira total per day, a gold daily-average line, the week total and the change against last week. It reads settled `deposit` rows from `ledger_entries`, with day boundaries in Africa/Lagos time.
- **Restyle:** every page now uses the theme roles and shared classes above, in both themes. The legacy colours (`forest`, `lime`, `azure`, `warn`) and the `.glass` classes are removed from `base.html`.

### Not built yet

- **Early withdrawal and liquidation** for flexible plans: a 3% fee taken on top, written to the ledger `fee` column. The **Withdraw** button on the plan page is disabled until then.
- **Scheduled jobs:** the 8:00 AM automatic add money, and the automatic payout at the target or end date.
- **Passbook streak grid** and plan history on the plan detail page.
- **Withdraw to a bank account.**
- **Groups, withdrawals and loans, notifications, linked bank accounts:** mock pages only. They are restyled but still show hardcoded sample data.
- **Admin side** and cooperatives hosting their own customers.
- **UI polish:** bolder naira amounts, and an animated splash and login (a pig logo with naira notes dropping into the slot).

### Known rough edges

- No "deposit received" page after paying on Paystack.
- A refresh or retry of the add money request could send it twice. The page blocks a double tap, but a one-time key on the form is the real fix.
- Ledger rows saved in one transaction share the same `created_at`, so their order inside one add money is not guaranteed.
- The minimum deposit (₦100) appears in three places: `deposits/services.rs`, `deposits/handlers.rs` and `deposit.html`.
- `profile.rs` keeps repo, service and handler in one file.
- `auth.rs` has its own `render` function that only accepts `LoginTemplate`.
- `LoanReviewTemplate` has a field named `Security` (style warning), and some mock templates have never-read fields (warnings).
- `deposits/handlers.rs` has an unreachable match pattern (warning).
- The mock group, rules, approvals and loan pages link to placeholder routes (`/rules`, `/approvals`, `/loans/new`, `/loans/review`, `/loans/detail`). The group list links to `/groups/founders-x89`, while the back arrows on the rules and approvals pages go to `/groups/founders`.

## Roadmap

1. Database: done
2. Auth and sessions: done
3. Plans pages and the ledger: done
4. Add money from wallet to plan: done
5. UI restyle (themes, roles, shared classes, deposits chart): done
6. Early withdrawal, liquidation and payout: next
7. Streak grid and plan history
8. Scheduled jobs (automatic add money, automatic payout)
9. Withdraw to bank and linked bank accounts
10. Groups and loans, then the admin side and cooperatives
11. UI polish (bolder amounts, animated splash and login)
12. Split templates into htmx fragments
