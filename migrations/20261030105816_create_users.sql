-- Add migration script here
CREATE TABLE users (
    user_id UUID PRIMARY KEY, 
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL, 
    hashed_password TEXT NOT NULL, 
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
