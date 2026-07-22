INSERT OR IGNORE INTO scheduled_jobs (name, spec, last_success_at, enabled)
VALUES ('nightly-curation', 'daily@00:00;catchup=20h', NULL, 1);
