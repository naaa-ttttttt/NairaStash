-- Add migration script here
CREATE TYPE transaction_type AS ENUM ('deposit', 'withdrawal');

CREATE TABLE transaction_history (
    transaction_id UUID PRIMARY KEY, 
    user_id UUID NOT NULL REFERENCES users(user_id),
    plan_id UUID NOT NULL REFERENCES savings_plans(plan_id),
    deposit_id UUID REFERENCES deposits(deposit_id),
    withdrawal_id UUID REFERENCES withdrawals(withdrawal_id),
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    status transaction_status NOT NULL,
    transaction_type transaction_type NOT NULL, 
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
