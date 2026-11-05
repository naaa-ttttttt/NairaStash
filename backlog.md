# Naira Stash - Sprint 1 Backlog: User Initiates Deposit

## 📦 Layer 1: Repository / DB Functions (Time: ~1.5 Hours)
- [ ] Write `createPendingDeposit(userId, amount, reference)` to log the initial intent in the database.
- [ ] Write `getDepositByReference(reference)` to look up a transaction later when verifying.
- [ ] Write `updateWalletBalance(userId, amount)` to increment the user's stored balance upon success.

## 🔌 Layer 2: Paystack External Gateway (Time: ~1.5 Hours)
- [ ] Set up Paystack API secret keys in a hidden environment file (`.env`).
- [ ] Write `initializePaystackPayment(email, amount)` inside a gateway file to call Paystack's API and get the secure payment URL.

## 🧠 Layer 3: Business Logic / Service Layer (Time: ~2 Hours)
- [ ] Write `startDepositProcess(userId, amount)` function.
- [ ] Add rule: Validate that the deposit amount meets the minimum required thrift limit (e.g., ₦100).
- [ ] Orchestrate the step: Fetch user email -> Call Paystack initialization -> Save the pending deposit log with the Paystack reference -> Return the checkout URL to the API layer.

## 🚦 Layer 4: API Routing & Webhooks (Time: ~1.5 Hours)
- [ ] Create `POST /deposits/initialize` path to take the payload `{ userId, amount }`, trigger the business logic, and send the checkout URL back to the client [2.1].
- [ ] Create `POST /deposits/webhook` path—this is the secure digital room where Paystack will send the success receipt later to trigger the wallet balance update [2.1].

## 🧪 Layer 5: Testing & Verification (Time: ~30 Mins)
- [ ] Use Postman to hit `POST /deposits/initialize` and verify you get back a working Paystack checkout link [2.1].
- [ ] Paste the link into your browser, run a test card payment, and verify your database status updates to "completed" and the balance increments!

