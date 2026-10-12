-- Near-duplicate detection compares descriptions by trigram similarity
-- (design §6.3).
CREATE EXTENSION IF NOT EXISTS pg_trgm;
