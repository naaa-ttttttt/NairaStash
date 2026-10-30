-- Add migration script here
CREATE TYPE plan_frequency AS ENUM ('daily', 'weekly', 'monthly');

CREATE TABLE savings_plans (
    plan_id UUID PRIMARY KEY NOT NULL, 
    user_id UUID NOT NULL REFERENCES users(user_id),
    amount NUMERIC(12, 2) NOT NULL,
    name TEXT NOT NULL,
    frequency plan_frequency NOT NULL,
    target NUMERIC(12, 2),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
