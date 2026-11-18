-- Add migration script here
-- the old table was made before the wallet-first design and nothing uses it
DROP TABLE transaction_history;
DROP TYPE transaction_type;

-- the four kinds of settled money movement
CREATE TYPE ledger_entry_type AS ENUM (
    'deposit',
    'plan_contribution',
    'plan_withdrawal',
    'payout'
);

-- one row per settled movement, rows are never changed or removed
CREATE TABLE ledger_entries (
    entry_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(user_id),
    entry_type ledger_entry_type NOT NULL,
    amount BIGINT NOT NULL CHECK (amount > 0),
    plan_id UUID REFERENCES savings_plans(plan_id),
    remark TEXT CHECK (remark IS NULL OR char_length(remark) <= 200),
    reference TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- plan entries must name a plan, wallet entries must not
    CONSTRAINT ledger_plan_matches_type CHECK (
        (entry_type IN ('plan_contribution', 'plan_withdrawal') AND plan_id IS NOT NULL)
        OR (entry_type IN ('deposit', 'payout') AND plan_id IS NULL)
    )
);

-- the activity page reads one user's entries, newest first
CREATE INDEX ledger_entries_user_created_idx ON ledger_entries (user_id, created_at DESC);

-- the plan page and the streak grid read one plan's entries
CREATE INDEX ledger_entries_plan_idx ON ledger_entries (plan_id) WHERE plan_id IS NOT NULL;

-- the database itself refuses to edit or delete a ledger row
CREATE FUNCTION ledger_entries_block_changes() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'ledger_entries is append-only, % is not allowed', TG_OP;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ledger_entries_no_update_delete
BEFORE UPDATE OR DELETE ON ledger_entries
FOR EACH ROW EXECUTE FUNCTION ledger_entries_block_changes();
