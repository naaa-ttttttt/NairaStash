# NairaStash

A mobile-first savings app for individuals and community savings groups, priced in Nigerian naira (₦, NGN).

NairaStash lets people save a fixed amount every day, boost their savings with extra deposits, withdraw when they need to, and join group savings circles (Esusu/chit style) where withdrawals and loans are approved by group admins.

> **Project status: static prototype.** Every screen runs on hardcoded mock data. There is no database, authentication or payment integration yet. The backend is built incrementally, so the frontend may change once real data is connected.

---

## Features

### Individual savings
- **Dashboard:** total balance, daily target progress, contribution status (ahead, on track, behind), active plans and recent inflows.
- **Savings plans:** a searchable list with filters (All, Ahead, On track, Behind) and a "catch up" prompt for missed deposits.
- **Plan details:** balance, daily rate, yield, a status preview, a 10-day timeline, boost buttons and a full-year passbook ledger.
- **New plan:** create a savings plan.
- **Deposit:** a 3-step flow (plan, amount, bank transfer).
- **Withdraw:** a 4-step flow (plan, amount, destination account, review) with live balance impact and over-balance validation.

### Savings groups
- **Groups list:** searchable group cards with filters (All, Active, Forming).
- **Group details:** pool stash, payout schedule, member roster and three tabs: Personal, Credit and Roster.
- **Group rules:** admins set withdrawal, loan and contribution rules (limits, pool fee, minimum balances, history required).
- **Admin approvals:** a queue of pending withdrawals and loans. Each request is checked against the rules, and a request that breaks a rule cannot be approved. Admins cannot approve their own requests.
- **Group loans:** a 2-step application (amount and repayment speed, then review) with a live fee and repayment calculation, followed by a loan details screen.
- **Loan lifecycle states:** active, waiting for admin, approved, paid off and declined.

### Account
- **Login and sign-up:** tabbed forms with a password strength meter.
- **Profile:** identity, security, banking, reminders, support and log out.
- **Notifications:** a filterable activity feed (savings, groups, loans, withdrawals) with unread toggle and "mark all read".

### Group approval model

| Action | Who approves |
|---|---|
| Withdraw from a personal plan | Instant, no approval |
| Withdraw from a group share | A group admin |
| Group loan (any amount) | A group admin |
| Changing group rules | Group admins only |

There is no member voting at this stage.

---

## Tech stack

| Layer | Technology |
|---|---|
| Language | Rust (edition 2024) |
| Web framework | Axum 0.7 on Tokio |
| Templates | Askama 0.12 (`askama_axum` 0.4) |
| Frontend | HTML5, Tailwind CSS (CDN), HTMX 2, Material Symbols icons |
| Fonts | Plus Jakarta Sans |
| Database (planned) | PostgreSQL with SQLx 0.8 |
| Other crates | serde, tower-http 0.5 (fs, cors) |

There is no React and no frontend build step. JavaScript is limited to small page scripts (tabs, filters, calculators).

---

## Getting started

### Prerequisites
- Rust toolchain (stable, with edition 2024 support) and Cargo

### Run locally

```bash
git clone https://github.com/naaa-ttttttt/NairaStash.git
cd NairaStash
cargo run
```

Open http://127.0.0.1:3000.

> Askama compiles templates into the binary. After changing any file in `templates/`, stop the server and run `cargo run` again.

---

## Routes

| Route | Page |
|---|---|
| `GET /` | Dashboard |
| `GET /login` | Log in and sign up |
| `POST /login`, `POST /signup` | Stubs, redirect to `/` |
| `GET /plans` | Plans list |
| `GET, POST /plans/new` | New plan (POST is a stub) |
| `GET /plans/:id` | Plan details |
| `GET /groups` | Groups list |
| `GET /groups/new` | Stub, redirects to `/groups` |
| `GET /groups/:id` | Group details |
| `GET /deposit` | Deposit flow |
| `GET /withdraw` | Withdraw step 1: select plan |
| `GET /withdraw/amount` | Withdraw step 2: amount |
| `GET /withdraw/destination` | Withdraw step 3: destination account |
| `GET /withdraw/review` | Withdraw step 4: review and confirm |
| `GET /loans/new` | Loan request |
| `GET /loans/review` | Loan review |
| `GET /loans/detail` | Loan details and states |
| `GET /rules` | Group rules |
| `GET /approvals` | Admin approvals |
| `GET /profile` | Profile |
| `GET /notifications` | Notifications |

Plan and group ids are accepted in the URL but ignored until real data exists. Multi-step flows pass their choices in the query string (for example `?plan=emergency&amount=120000`).

---

## Project structure

```
savings_app/
├── Cargo.toml
├── src/
│   └── main.rs              # routes, handlers and Askama template structs
└── templates/
    ├── base.html            # layout, theme, bottom navigation
    ├── dashboard.html
    ├── plans.html
    ├── new_plan.html
    ├── plan_detail.html
    ├── groups.html
    ├── group_detail.html
    ├── group_rules.html
    ├── group_approvals.html
    ├── deposit.html
    ├── withdraw_plan.html
    ├── withdraw_amount.html
    ├── withdraw_destination.html
    ├── withdraw_review.html
    ├── loan_request.html
    ├── loan_review.html
    ├── loan_detail.html
    ├── login.html
    ├── profile.html
    └── notifications.html
```

---

## Design system

Defined in `templates/base.html`.

- **Look:** dark green gradient background with glass-style cards (`.glass`, `.glass-dark`).
- **Colors:** lime `#D8F13A` (primary actions, good status), lime-ink `#10301C` (text on lime), azure `#2F7CF6` (on track), warn `#FFC247` (behind or warning), and a forest green scale.
- **Patterns:** `rounded-3xl` cards, `rounded-full` pills and buttons, a floating bottom navigation pill, initials avatars instead of photos.
- **Page conventions:** each page extends `base.html`, defines `title` and `content` blocks, and uses `<!-- fragment: name -->` comments so templates can later be split into HTMX partials. Pages with their own action bar (login, withdraw, loan flows) hide the bottom nav through the `nav` block.

---

## Currency and locale

All amounts are in naira (₦, NGN). Mock data uses Nigerian banks (GTBank, Access Bank), NIP instant transfers, BVN/NIN identity wording and WAT times. No dollar amounts are used anywhere.

---

## Known limitations

- All data is mock data, so nothing persists and balances are static.
- Login, sign-up and plan creation only redirect.
- Group rule numbers on the loan, withdrawal and approvals pages are hardcoded, so changing a rule on the rules page does not change them.
- The Activity tab in the bottom navigation is a placeholder.
- The "New group" screen is not designed yet.
- Some floating action buttons (such as "Instant deposit" on group details) overlap the bottom navigation.

---

## Roadmap

1. Connect PostgreSQL with SQLx and replace mock data with real records.
2. Real authentication and sessions.
3. Activity page and a "new group" flow.
4. Admin side of the platform.
5. Payment and bank transfer integration.
6. Split templates into HTMX fragments for partial updates.

---

## Contributing

This is an actively developed personal project. Issues and suggestions are welcome through GitHub.

## License

Not yet specified.
