-- Add migration script here
CREATE TABLE wallets (
    wallet_id UUID PRIMARY KEY,
    user_id UUID NOT NULL UNIQUE REFERENCES users(user_id),
    balance BIGINT NOT NULL DEFAULT 0,
    currency VARCHAR(3) NOT NULL DEFAULT 'NGN' CHECK (currency = 'NGN')
)
