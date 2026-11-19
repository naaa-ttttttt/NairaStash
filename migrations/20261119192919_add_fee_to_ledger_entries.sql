-- Add migration script here
-- the early withdrawal fee in kobo, 0 for every row that has no fee
ALTER TABLE ledger_entries
    ADD COLUMN fee BIGINT NOT NULL DEFAULT 0;

-- a fee can never be negative
ALTER TABLE ledger_entries
    ADD CONSTRAINT ledger_entries_fee_check CHECK (fee >= 0);

-- only a plan withdrawal can carry a fee
ALTER TABLE ledger_entries
    ADD CONSTRAINT ledger_fee_only_on_withdrawal
    CHECK (fee = 0 OR entry_type = 'plan_withdrawal');
