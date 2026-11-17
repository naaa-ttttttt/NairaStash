-- Add migration script here
-- a wallet deposit belongs to no plan, the money lands in the wallet first
ALTER TABLE deposits DROP COLUMN plan_id;

-- money in whole kobo, like the rest of the app
ALTER TABLE withdrawals
    ALTER COLUMN amount TYPE BIGINT USING (amount * 100)::BIGINT;
ALTER TABLE transaction_history
    ALTER COLUMN amount TYPE BIGINT USING (amount * 100)::BIGINT;
