-- Add migration script here
-- when the session was last used, the idle timeout counts from here
ALTER TABLE sessions ADD COLUMN last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now();
