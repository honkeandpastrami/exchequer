-- Sessions apart from tokens (design §10). A session is a client using a
-- token: a browser or the CLI signed in with a passkey, or a script with a
-- token made for it. Only scripts' tokens have names.

-- The old rules go before the rows change, the new ones after.
ALTER TABLE api_token
    DROP CONSTRAINT api_token_kind_check,
    ALTER COLUMN name DROP NOT NULL;
UPDATE api_token SET kind = 'browser' WHERE kind = 'session';
UPDATE api_token SET name = NULL WHERE kind <> 'script';
ALTER TABLE api_token
    ADD CHECK (kind IN ('browser', 'cli', 'script')),
    ADD CHECK ((kind = 'script') = (name IS NOT NULL)),
    -- The passkey a browser signed in with, or the CLI's approver did.
    ADD COLUMN passkey_id uuid REFERENCES passkey ON DELETE SET NULL;

ALTER TABLE device_sign_in
    ADD COLUMN passkey_id uuid REFERENCES passkey ON DELETE SET NULL;

-- Each client using a token, as it describes itself: its User-Agent and the
-- host it sends (`Exchequer-Host`), and the address the request came from.
-- A browser's or the CLI's token is one session; a script's token has one
-- per tool and host (`key`).
CREATE TABLE token_session (
    id           uuid PRIMARY KEY,
    token_id     uuid NOT NULL REFERENCES api_token,
    key          text NOT NULL,
    agent        text,
    host         text,
    ip           text,
    started_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (token_id, key)
);

-- Browsers and CLIs signed in now keep working, as sessions from an unknown
-- client.
INSERT INTO token_session (id, token_id, key, started_at, last_used_at)
    SELECT gen_random_uuid(), id, '', created_at, coalesce(last_used_at, created_at)
    FROM api_token
    WHERE kind <> 'script' AND revoked_at IS NULL AND expires_at > now();

-- An invite only adds a passkey.
ALTER TABLE invite DROP COLUMN replace;
