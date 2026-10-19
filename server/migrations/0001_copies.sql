-- One encrypted copy per sync key. data is base64; NULL once deleted.
CREATE TABLE copies (
    id TEXT PRIMARY KEY,
    version INTEGER NOT NULL,
    data TEXT,
    updated_at INTEGER NOT NULL
);
CREATE INDEX copies_by_age ON copies (updated_at);
