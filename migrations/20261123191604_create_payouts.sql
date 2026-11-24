-- Add migration script here
-- a payout is money going from a wallet to a saved bank account, pending until paystack says how it ended
CREATE TYPE payout_status AS ENUM ('pending', 'success', 'failed');

CREATE TABLE payouts (
    payout_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(user_id),
    -- removed banks keep their row (soft delete), so this link never breaks
    bank_account_id UUID NOT NULL REFERENCES bank_accounts(bank_account_id),
    -- what the bank receives, in kobo
    amount BIGINT NOT NULL CHECK (amount > 0),
    -- what we charge the user on top, in kobo (paystack fee plus stamp duty)
    fee BIGINT NOT NULL CHECK (fee >= 0),
    -- our own unique reference, sent to paystack so one payout can never be sent twice
    reference TEXT NOT NULL UNIQUE,
    -- paystack's code for the transfer, filled once paystack accepts it
    transfer_code TEXT,
    status payout_status NOT NULL DEFAULT 'pending',
    failure_reason TEXT,
    -- the wallet balance right after the hold, used for the ledger row when the payout succeeds
    wallet_balance_after BIGINT NOT NULL CHECK (wallet_balance_after >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    settled_at TIMESTAMPTZ,
    -- settled_at is filled exactly when the payout is no longer pending
    CONSTRAINT payouts_settled_matches_status
        CHECK ((status = 'pending') = (settled_at IS NULL))
);

-- the daily limit check and the payouts page both read one user's recent payouts
CREATE INDEX payouts_user_created_idx ON payouts (user_id, created_at DESC);

-- the stuck-payout job only looks at pending rows
CREATE INDEX payouts_pending_idx ON payouts (created_at) WHERE status = 'pending';
