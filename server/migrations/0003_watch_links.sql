-- Garmin watches and the phones paired with them (see src/watch.js). Keys
-- are public and the items encrypted on the phone; keys, and each item,
-- are base64; items is a JSON array of them. The two secrets are kept only
-- as SHA-256 hashes: token_hash the watch's, writer_hash the phone's.
--
-- A row starts as a pairing: code set, app_key, writer_hash, version and
-- items NULL until the phone has sent them, done NULL until the watch has
-- answered (used, declined, unreadable or left). A pairing the watch took
-- (done = 'used') stays as the link: its code goes at once, and the row when
-- either side forgets it or neither has used it for 400 days. Any other
-- pairing goes ten minutes after it was made, or after the app sent to it.
CREATE TABLE watch_links (
    token_hash TEXT PRIMARY KEY,
    code TEXT UNIQUE,
    watch_key TEXT NOT NULL,
    app_key TEXT,
    writer_hash TEXT UNIQUE,
    version INTEGER,
    items TEXT,
    done TEXT,
    -- What the watch last said it has: the version it keeps, and used or
    -- unreadable for the last one it fetched.
    watch_version INTEGER,
    watch_said TEXT,
    created_at INTEGER NOT NULL,
    used_at INTEGER NOT NULL
);
CREATE INDEX watch_links_by_age ON watch_links (created_at);
CREATE INDEX watch_links_by_use ON watch_links (used_at);
