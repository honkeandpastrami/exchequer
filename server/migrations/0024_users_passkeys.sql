-- Users sign in with passkeys (design §10). Nothing secret is stored: a
-- passkey is its public key, and invites, device codes and tokens are hashed.

CREATE TABLE app_user (
    id         uuid PRIMARY KEY,
    name       text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Removed users keep their row: the audit log refers to their tokens.
    removed_at timestamptz
);

-- A token belongs to a user (a session, the CLI's, or one they created), or
-- to no one (scripts, made with the bootstrap token). Sessions expire.
ALTER TABLE api_token
    ADD COLUMN user_id    uuid REFERENCES app_user,
    ADD COLUMN expires_at timestamptz;

CREATE TABLE passkey (
    id            uuid PRIMARY KEY,
    user_id       uuid NOT NULL REFERENCES app_user,
    credential_id bytea NOT NULL UNIQUE,
    name          text NOT NULL,
    -- webauthn-rs's Passkey: the credential ID, public key and counter.
    credential    jsonb NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_used_at  timestamptz
);
CREATE INDEX passkey_user ON passkey (user_id);

-- One-time links that register a passkey for a user.
CREATE TABLE invite (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES app_user,
    secret_hash text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    expires_at  timestamptz NOT NULL,
    used_at     timestamptz
);

-- A registration or sign-in in progress: its challenge, used once.
CREATE TABLE auth_ceremony (
    id         uuid PRIMARY KEY,
    kind       text NOT NULL CHECK (kind IN ('register', 'sign_in')),
    invite_id  uuid REFERENCES invite,
    state      jsonb NOT NULL,
    expires_at timestamptz NOT NULL,
    CHECK ((kind = 'register') = (invite_id IS NOT NULL))
);

-- The CLI signing in through the browser: it holds the device code, a
-- signed-in user approves the user code, and the CLI then gets its token.
CREATE TABLE device_sign_in (
    id          uuid PRIMARY KEY,
    secret_hash text NOT NULL,
    user_code   text NOT NULL UNIQUE,
    client      text NOT NULL,
    expires_at  timestamptz NOT NULL,
    approved_by uuid REFERENCES app_user
);
