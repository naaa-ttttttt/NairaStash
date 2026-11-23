-- Add migration script here
-- two users can save the same account, and paystack may return the same recipient code for it
ALTER TABLE bank_accounts DROP CONSTRAINT bank_accounts_recipient_code_key;
