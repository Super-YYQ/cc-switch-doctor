PRAGMA user_version = 16;

-- Endpoints table missing; one provider has Base URL in settings, one does not.
CREATE TABLE providers (
    id TEXT NOT NULL,
    app_type TEXT NOT NULL,
    name TEXT NOT NULL,
    settings_config TEXT NOT NULL,
    website_url TEXT,
    category TEXT,
    created_at INTEGER,
    sort_index INTEGER,
    notes TEXT,
    icon TEXT,
    icon_color TEXT,
    meta TEXT NOT NULL DEFAULT '{}',
    is_current BOOLEAN NOT NULL DEFAULT 0,
    in_failover_queue BOOLEAN NOT NULL DEFAULT 0,
    PRIMARY KEY (id, app_type)
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT
);

-- Has Base URL in settings_config → still testable via degraded endpoint scan.
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'ep-missing-ok',
  'claude',
  'Settings Has Base URL',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.ep-missing.test/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-ep-missing-ok","ANTHROPIC_MODEL":"glm-ep"}}',
  'https://ep-missing.test',
  'custom',
  1710000400,
  1,
  'fixture',
  'anthropic',
  '#D97706',
  '{"apiFormat":"anthropic"}',
  1,
  0
);

-- No Base URL → that provider alone is skipped (MissingBaseUrl).
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'ep-missing-no-url',
  'claude',
  'Settings No Base URL',
  '{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-ep-missing-nourl"}}',
  NULL,
  'custom',
  1710000401,
  2,
  'fixture',
  'anthropic',
  '#D97706',
  '{}',
  0,
  0
);
