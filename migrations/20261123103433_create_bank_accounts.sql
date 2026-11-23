-- Add migration script here
-- a bank account the user saved to receive payouts, removing one is a soft delete so payout history keeps its link
CREATE TYPE bank_account_status AS ENUM ('active', 'removed');

CREATE TABLE bank_accounts (
    bank_account_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(user_id),
    bank_name TEXT NOT NULL,
    bank_code TEXT NOT NULL,
    -- text, not a number, because account numbers can start with zero
    account_number TEXT NOT NULL CHECK (account_number ~ '^[0-9]{10}$'),
    -- the name paystack resolved, saved exactly as returned
    account_name TEXT NOT NULL,
    recipient_code TEXT NOT NULL UNIQUE,
    status bank_account_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    removed_at TIMESTAMPTZ,
    -- removed_at is filled exactly when the bank is removed
    CONSTRAINT bank_accounts_removed_matches_status
        CHECK ((status = 'removed') = (removed_at IS NOT NULL))
);

-- one user cannot have the same account active twice, a removed one can be saved again
CREATE UNIQUE INDEX bank_accounts_one_active_per_user
    ON bank_accounts (user_id, bank_code, account_number)
    WHERE status = 'active';
