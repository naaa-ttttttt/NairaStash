-- Add migration script here
-- one pin per user, used to approve payouts
CREATE TABLE payout_pins (
    user_id UUID PRIMARY KEY REFERENCES users(user_id),
    pin_hash TEXT NOT NULL,
    -- wrong pin tries since the last correct one, and when the lock ends
    failed_attempts INTEGER NOT NULL DEFAULT 0 CHECK (failed_attempts >= 0),
    locked_until TIMESTAMPTZ,
    -- the same two counters for wrong passwords on the forgot-pin page
    reset_failed_attempts INTEGER NOT NULL DEFAULT 0 CHECK (reset_failed_attempts >= 0),
    reset_locked_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
