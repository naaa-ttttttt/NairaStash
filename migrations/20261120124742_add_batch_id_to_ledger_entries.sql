-- Add migration script here
-- one id per add money tap, rows with the same batch_id belong together
-- old rows keep NULL and stay single lines on the activity page
ALTER TABLE ledger_entries ADD COLUMN batch_id UUID NULL;

-- the detail page sums a whole batch, so lookups by batch_id must be fast
CREATE INDEX idx_ledger_entries_batch_id ON ledger_entries (batch_id)
    WHERE batch_id IS NOT NULL;
