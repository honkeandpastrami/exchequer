-- Every token expires, belongs to a user, and records when it was last used
-- (design §10). The bootstrap token no longer makes tokens.

ALTER TABLE api_token
    ADD COLUMN kind         text,
    ADD COLUMN last_used_at timestamptz;

-- Sessions were the tokens that expired; the CLI's were named
-- `<user> · <client> · <6 hex>`; the rest were made by hand.
UPDATE api_token SET kind = CASE
    WHEN expires_at IS NOT NULL THEN 'session'
    WHEN user_id IS NOT NULL AND name ~ ' · .+ · [0-9a-f]{6}$' THEN 'cli'
    ELSE 'script'
END;

-- Tokens nobody owned (made with the bootstrap token) go to the first user;
-- with no user, they stop working. Tokens that lasted forever now last 90
-- days.
UPDATE api_token
    SET user_id = (SELECT id FROM app_user WHERE removed_at IS NULL
                   ORDER BY created_at, id LIMIT 1)
    WHERE user_id IS NULL;
UPDATE api_token SET revoked_at = coalesce(revoked_at, now()) WHERE user_id IS NULL;
UPDATE api_token SET expires_at = now() + interval '90 days' WHERE expires_at IS NULL;

ALTER TABLE api_token
    ALTER COLUMN kind SET NOT NULL,
    ALTER COLUMN expires_at SET NOT NULL,
    ADD CHECK (kind IN ('session', 'cli', 'script')),
    ADD CHECK (user_id IS NOT NULL OR revoked_at IS NOT NULL);

-- The CLI asks for its token's time to live; the approver sees it. Sign-ins
-- in progress last minutes, so the ones under way are simply dropped.
DELETE FROM device_sign_in;
ALTER TABLE device_sign_in
    ADD COLUMN ttl_days integer NOT NULL CHECK (ttl_days > 0);
