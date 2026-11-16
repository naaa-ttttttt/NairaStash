-- Add migration script here
-- two new types
CREATE TYPE lock_mode AS ENUM ('locked', 'flexible');
CREATE TYPE plan_status AS ENUM ('active', 'closed');

-- rename the columns so the names say what they mean
ALTER TABLE savings_plans RENAME COLUMN amount TO amount_per_frequency;
ALTER TABLE savings_plans RENAME COLUMN target TO target_amount;

-- money in whole kobo, like deposits and wallets
ALTER TABLE savings_plans
    ALTER COLUMN amount_per_frequency TYPE BIGINT USING (amount_per_frequency * 100)::BIGINT,
    ALTER COLUMN target_amount TYPE BIGINT USING (target_amount * 100)::BIGINT;

-- the new fields
ALTER TABLE savings_plans
    ADD COLUMN lock_mode lock_mode NOT NULL DEFAULT 'flexible',
    ADD COLUMN end_date DATE,
    ADD COLUMN balance BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN status plan_status NOT NULL DEFAULT 'active';

-- rules the database enforces itself
ALTER TABLE savings_plans
    ADD CONSTRAINT amount_per_frequency_positive CHECK (amount_per_frequency > 0),
    ADD CONSTRAINT target_amount_positive CHECK (target_amount IS NULL OR target_amount > 0),
    ADD CONSTRAINT balance_not_negative CHECK (balance >= 0),
    ADD CONSTRAINT locked_needs_end_date CHECK (
        (lock_mode = 'locked' AND end_date IS NOT NULL)
        OR (lock_mode = 'flexible' AND end_date IS NULL)
    );
