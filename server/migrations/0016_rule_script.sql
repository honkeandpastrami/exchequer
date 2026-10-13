-- Rules (design §8, docs/ai.md): the household's Rhai script, kept as
-- versions, never edited or deleted. The newest is active; only scripts
-- that passed validation are stored. With no version, there are no rules.
CREATE TABLE rule_script (
    version    integer PRIMARY KEY CHECK (version > 0),
    source     text NOT NULL CHECK (length(source) <= 200000),
    note       text CHECK (note <> '' AND length(note) <= 500),
    -- Who uploaded it: the token, if any (not the bootstrap token).
    token_id   uuid REFERENCES api_token,
    created_at timestamptz NOT NULL DEFAULT now()
);
