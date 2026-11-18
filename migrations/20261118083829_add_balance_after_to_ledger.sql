-- Add migration script here
-- the wallet balance right after this movement, like a bank statement line
ALTER TABLE ledger_entries
    ADD COLUMN wallet_balance_after BIGINT NOT NULL CHECK (wallet_balance_after >= 0);
