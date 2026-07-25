PRAGMA user_version = 16;

-- Three providers: two valid, one with unparseable settings_config.
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

CREATE TABLE provider_endpoints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    app_type TEXT NOT NULL,
    url TEXT NOT NULL,
    added_at INTEGER
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT
);

INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'ok-a',
  'claude',
  'Valid Provider A',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.ok-a.test/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-ok-a-only","ANTHROPIC_MODEL":"glm-a"}}',
  'https://ok-a.test',
  'custom',
  1710000600,
  1,
  'fixture',
  'anthropic',
  '#D97706',
  '{"apiFormat":"anthropic"}',
  1,
  0
);

INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'bad-settings',
  'claude',
  'Invalid Settings Provider',
  'this-is-not-json{{{',
  'https://bad.test',
  'custom',
  1710000601,
  2,
  'fixture',
  'anthropic',
  '#D97706',
  '{}',
  0,
  0
);

INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'ok-b',
  'claude',
  'Valid Provider B',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.ok-b.test/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-ok-b-only","ANTHROPIC_MODEL":"glm-b"}}',
  'https://ok-b.test',
  'custom',
  1710000602,
  3,
  'fixture',
  'anthropic',
  '#D97706',
  '{"apiFormat":"anthropic"}',
  0,
  0
);

INSERT INTO provider_endpoints (provider_id, app_type, url, added_at) VALUES
  ('ok-a', 'claude', 'https://api.ok-a.test/v1', 1710000600),
  ('ok-b', 'claude', 'https://api.ok-b.test/v1', 1710000602);
