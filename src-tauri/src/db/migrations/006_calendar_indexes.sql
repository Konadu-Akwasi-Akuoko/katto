-- Enforce one shoot + one publish per project (dedupe any legacy rows first),
-- and index the calendar's three range queries.
DELETE FROM schedule
WHERE id NOT IN (SELECT MAX(id) FROM schedule GROUP BY project_slug, kind);
CREATE UNIQUE INDEX idx_schedule_project_kind ON schedule(project_slug, kind);
CREATE INDEX idx_schedule_date   ON schedule(date);
CREATE INDEX idx_events_kind_ts  ON events(kind, ts);
CREATE INDEX idx_ideas_first_seen ON ideas(first_seen);
