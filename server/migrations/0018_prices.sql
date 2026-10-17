-- Prices (design §3.4): `1 base = rate quote` at the end of a day, from one
-- source. A source's prices outlive it: retiring one leaves them in place.

CREATE TABLE price (
    id          uuid PRIMARY KEY,
    base_id     uuid NOT NULL REFERENCES commodity ON DELETE CASCADE,
    quote_id    uuid NOT NULL REFERENCES commodity ON DELETE CASCADE,
    date        date NOT NULL,
    rate        numeric NOT NULL CHECK (rate > 0 AND scale(rate) <= 20),
    source      text NOT NULL
                CHECK (source = 'manual' OR source ~ '^provider:[a-z0-9-]{1,32}$'),
    -- A day still in progress: replaced by the close once available.
    provisional boolean NOT NULL DEFAULT false,
    fetched_at  timestamptz NOT NULL DEFAULT now(),
    version     bigint NOT NULL DEFAULT 1,
    CHECK (base_id <> quote_id),
    UNIQUE (base_id, quote_id, source, date)
);
-- Looked up either way round.
CREATE INDEX price_inverse ON price (quote_id, base_id, date);
