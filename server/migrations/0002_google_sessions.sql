-- Google sign-ins waiting for the app to pick up their tokens (minutes).
CREATE TABLE google_sessions (
    id TEXT PRIMARY KEY,
    tokens TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
