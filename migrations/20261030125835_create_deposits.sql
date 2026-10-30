-- Add migration script here
CREATE TYPE transaction_status AS ENUM ('successful', 'pending', 'failed');
CREATE TYPE deposit_type AS ENUM ('auto_debit', 'manual');
CREATE TABLE deposits (
    deposit_id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(user_id) NOT NULL,
    plan_id UUID NOT NULL REFERENCES savings_plans(plan_id),
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    status transaction_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deposit_type deposit_type NOT NULL
);
