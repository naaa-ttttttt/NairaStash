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
# NairaStash

A mobile-first savings app for individuals and community savings groups, priced in Nigerian naira (₦, NGN).

NairaStash lets people save a fixed amount every day, boost their savings with extra deposits, withdraw when they need to, and join group savings circles (Esusu/chit style) where withdrawals and loans are approved by group admins.

> **Project status: static prototype.** Every screen runs on hardcoded mock data. There is no database, authentication or payment integration yet. The backend is built incrementally, so the frontend may change once real data is connected.

---

## Features

### Individual savings
- **Dashboard:** total balance, daily target progress, contribution status (ahead, on track, behind), active plans and recent inflows.
- **Savings plans:** a searchable list with filters (All, Ahead, On track, Behind) and a catch up prompt for missed deposits.
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
- **Notifications:** a filterable activity feed (savings, groups, loans, withdrawals) with unread toggle and mark all read.

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
| Templates | Askama 0.12 ( 0.4) |
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

]666;vte.shell.postexec=0\]7;file://naaa-ttttttt/home/natty/savings_app\]0;natty@naaa-ttttttt:~/savings_app\]666;vte.shell.precmd!\[?1049h[22;0;0t[>4;2m[?1h=[?2004h[?1004h[1;38r[?12h[?12l[22;2t[22;1t[27m[23m[29m[m[H[2J[?25l[38;1H"README.md" 1L, 13B[1;1H[38;5;130m  1 [m[35m# [m[35mNairaStash[m
[94m~                                                                                                                                                                                             [3;1H~                                                                                                                                                                                             [4;1H~                                                                                                                                                                                             [5;1H~                                                                                                                                                                                             [6;1H~                                                                                                                                                                                             [7;1H~                                                                                                                                                                                             [8;1H~                                                                                                                                                                                             [9;1H~                                                                                                                                                                                             [10;1H~                                                                                                                                                                                             [11;1H~                                                                                                                                                                                             [12;1H~                                                                                                                                                                                             [13;1H~                                                                                                                                                                                             [14;1H~                                                                                                                                                                                             [15;1H~                                                                                                                                                                                             [16;1H~                                                                                                                                                                                             [17;1H~                                                                                                                                                                                             [18;1H~                                                                                                                                                                                             [19;1H~                                                                                                                                                                                             [20;1H~                                                                                                                                                                                             [21;1H~                                                                                                                                                                                             [22;1H~                                                                                                                                                                                             [23;1H~                                                                                                                                                                                             [24;1H~                                                                                                                                                                                             [25;1H~                                                                                                                                                                                             [26;1H~                                                                                                                                                                                             [27;1H~                                                                                                                                                                                             [28;1H~                                                                                                                                                                                             [29;1H~                                                                                                                                                                                             [30;1H~                                                                                                                                                                                             [31;1H~                                                                                                                                                                                             [32;1H~                                                                                                                                                                                             [33;1H~                                                                                                                                                                                             [34;1H~                                                                                                                                                                                             [35;1H~                                                                                                                                                                                             [36;1H~                                                                                                                                                                                             [37;1H~                                                                                                                                                                                             [m[38;173H1,1[11CAll[1;5H[?25h[?4m[?25l[38;163Hl[1;5H[38;163H [1;6H[38;175H2[1;6H[?25h[?25l[38;163H^M[1;6H[38;163H  [1;6H[?25h[38;1H[?2004l[>4;m[23;2t[23;1t[38;1H[K[38;1H[?1004l[?2004l[?1l>[?1049l[23;0;0t[>4;m]666;vte.shell.postexec=0\]7;file://naaa-ttttttt/home/natty/savings_app\]0;natty@naaa-ttttttt:~/savings_app\]666;vte.shell.precmd!\Cargo.lock  Cargo.toml	README.md  src/	 target/  templates/
]666;vte.shell.postexec=0\]7;file://naaa-ttttttt/home/natty/savings_app\]0;natty@naaa-ttttttt:~/savings_app\]666;vte.shell.precmd!\Cargo.lock  Cargo.toml	README.md  src/	 target/  templates/
]666;vte.shell.postexec=0\]7;file://naaa-ttttttt/home/natty/savings_app\]0;natty@naaa-ttttttt:~/savings_app\]666;vte.shell.precmd!\]666;vte.shell.postexec=0\]7;file://naaa-ttttttt/home/natty/savings_app/src\]0;natty@naaa-ttttttt:~/savings_app/src\]666;vte.shell.precmd!\main.rs
]666;vte.shell.postexec=0\]7;file://naaa-ttttttt/home/natty/savings_app/src\]0;natty@naaa-ttttttt:~/savings_app/src\]666;vte.shell.precmd!\]666;vte.shell.postexec=127\]7;file://naaa-ttttttt/home/natty/savings_app/src\]0;natty@naaa-ttttttt:~/savings_app/src\]666;vte.shell.precmd!\]666;vte.shell.postexec=0\]7;file://naaa-ttttttt/home/natty/savings_app/src\]0;natty@naaa-ttttttt:~/savings_app/src\]666;vte.shell.precmd!\]666;vte.shell.postexec=127\]7;file://naaa-ttttttt/home/natty/savings_app/src\]0;natty@naaa-ttttttt:~/savings_app/src\]666;vte.shell.precmd!\]666;vte.shell.postexec=2\]7;file://naaa-ttttttt/home/natty/savings_app/src\]0;natty@naaa-ttttttt:~/savings_app/src\]666;vte.shell.precmd!\

Open http://127.0.0.1:3000.

> Askama compiles templates into the binary. After changing any file in , stop the server and run  again.

---

## Routes

| Route | Page |
|---|---|
| <HTML>
<HEAD>
<TITLE>Directory /</TITLE>
<BASE HREF="file:/">
</HEAD>
<BODY>
<H1>Directory listing of /</H1>
<UL>
<LI><A HREF="./">./</A>
<LI><A HREF="../">../</A>
<LI><A HREF="bin/">bin/</A>
<LI><A HREF="boot/">boot/</A>
<LI><A HREF="cdrom/">cdrom/</A>
<LI><A HREF="debug">debug</A>
<LI><A HREF="dev/">dev/</A>
<LI><A HREF="etc/">etc/</A>
<LI><A HREF="home/">home/</A>
<LI><A HREF="lib/">lib/</A>
<LI><A HREF="lib32/">lib32/</A>
<LI><A HREF="lib64/">lib64/</A>
<LI><A HREF="lost%2Bfound/">lost+found/</A>
<LI><A HREF="media/">media/</A>
<LI><A HREF="mnt/">mnt/</A>
<LI><A HREF="opt/">opt/</A>
<LI><A HREF="proc/">proc/</A>
<LI><A HREF="root/">root/</A>
<LI><A HREF="run/">run/</A>
<LI><A HREF="sbin/">sbin/</A>
<LI><A HREF="snap/">snap/</A>
<LI><A HREF="srv/">srv/</A>
<LI><A HREF="swap.img">swap.img</A>
<LI><A HREF="sys/">sys/</A>
<LI><A HREF="timeshift/">timeshift/</A>
<LI><A HREF="tmp/">tmp/</A>
<LI><A HREF="usr/">usr/</A>
<LI><A HREF="var/">var/</A>
</UL>
</BODY>
</HTML> | Dashboard |
|  | Log in and sign up |
| ,  | Stubs, redirect to  |
|  | Plans list |
|  | New plan (POST is a stub) |
|  | Plan details |
|  | Groups list |
|  | Stub, redirects to  |
|  | Group details |
|  | Deposit flow |
|  | Withdraw step 1: select plan |
|  | Withdraw step 2: amount |
|  | Withdraw step 3: destination account |
|  | Withdraw step 4: review and confirm |
|  | Loan request |
|  | Loan review |
|  | Loan details and states |
|  | Group rules |
|  | Admin approvals |
|  | Profile |
|  | Notifications |

Plan and group ids are accepted in the URL but ignored until real data exists. Multi-step flows pass their choices in the query string (for example ).

---

## Project structure



---

## Design system

Defined in .

- **Look:** dark green gradient background with glass-style cards (, ).
- **Colors:** lime  (primary actions, good status), lime-ink  (text on lime), azure  (on track), warn  (behind or warning), and a forest green scale.
- **Patterns:**  cards,  pills and buttons, a floating bottom navigation pill, initials avatars instead of photos.

---

## Currency and locale

All amounts are in naira (₦, NGN). Mock data uses Nigerian banks (GTBank, Access Bank), NIP instant transfers, BVN/NIN identity wording and WAT times. No dollar amounts are used anywhere.

---

## Known limitations

- All data is mock data, so nothing persists and balances are static.
- Login, sign-up and plan creation only redirect.
- Group rule numbers on the loan, withdrawal and approvals pages are hardcoded, so changing a rule on the rules page does not change them.
- The Activity tab in the bottom navigation is a placeholder.
- The New group screen is not designed yet.
- Some floating action buttons (such as Instant deposit on group details) overlap the bottom navigation.

---

## Roadmap

1. Connect PostgreSQL with SQLx and replace mock data with real records.
2. Real authentication and sessions.
3. Activity page and a new group flow.
4. Admin side of the platform.
5. Payment and bank transfer integration.
6. Split templates into HTMX fragments for partial updates.

---

## Contributing

This is an actively developed personal project. Issues and suggestions are welcome through GitHub.

## License

Not yet specified.
