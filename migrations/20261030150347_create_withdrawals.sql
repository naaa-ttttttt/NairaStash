-- Add migration script here

CREATE TABLE withdrawals (
    withdrawal_id UUID PRIMARY KEY NOT NULL,
    user_id UUID NOT NULL REFERENCES users(user_id),
    plan_id UUID NOT NULL REFERENCES savings_plans(plan_id),
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    status transaction_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
