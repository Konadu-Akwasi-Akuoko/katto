CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE projects (
  slug         TEXT PRIMARY KEY,
  title        TEXT NOT NULL,
  root_path    TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'idea',
  target_nle   TEXT NOT NULL DEFAULT 'fcp',
  shoot_date   TEXT,
  publish_date TEXT,
  created_at   TEXT NOT NULL
);

-- ideas & raw_signal: column parity with hyper-frames tools/studio (D7).
CREATE TABLE ideas (
  id            TEXT PRIMARY KEY,
  type          TEXT NOT NULL,                 -- mirror|comment_demand|trend|manual
  kind          TEXT NOT NULL DEFAULT 'unset', -- unset|long|short|series
  status        TEXT NOT NULL DEFAULT 'backlog', -- backlog|promoted|discarded
  title         TEXT NOT NULL,
  rationale     TEXT,
  source        TEXT,
  source_url    TEXT,
  source_title  TEXT,
  evidence_json TEXT,
  raw_signal_id TEXT,
  first_seen    TEXT NOT NULL,
  notes         TEXT,
  promoted_slug TEXT,
  kind_source   TEXT,                          -- 'ai' | 'human'
  kind_why      TEXT
);
CREATE INDEX idx_ideas_status ON ideas(status);

CREATE TABLE raw_signal (
  id             TEXT PRIMARY KEY,
  source         TEXT NOT NULL,
  external_id    TEXT NOT NULL,
  title          TEXT,
  url            TEXT,
  payload_json   TEXT NOT NULL,
  fetched_at     TEXT NOT NULL,
  judged_at      TEXT,
  judged_verdict TEXT
);
CREATE INDEX idx_raw_unjudged ON raw_signal(judged_at) WHERE judged_at IS NULL;

CREATE TABLE schedule (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  project_slug TEXT NOT NULL REFERENCES projects(slug) ON DELETE CASCADE,
  kind         TEXT NOT NULL,                  -- shoot|publish
  date         TEXT NOT NULL,
  note         TEXT
);

CREATE TABLE events (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  ts           TEXT NOT NULL,
  kind         TEXT NOT NULL,
  project_slug TEXT,
  payload_json TEXT
);

CREATE TABLE jobs (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL,
  label        TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'queued', -- queued|running|done|failed
  progress     REAL NOT NULL DEFAULT 0,
  payload_json TEXT,
  error        TEXT,
  started_at   TEXT,
  finished_at  TEXT
);

CREATE TABLE scheduled_jobs (
  name            TEXT PRIMARY KEY,
  spec            TEXT NOT NULL,
  last_success_at TEXT,
  enabled         INTEGER NOT NULL DEFAULT 1
);
