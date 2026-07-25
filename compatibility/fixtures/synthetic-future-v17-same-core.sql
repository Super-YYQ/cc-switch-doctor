PRAGMA user_version = 17;

-- Future CC Switch version with identical Provider core structure.
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

INSERT INTO settings (key, value) VALUES ('app_version', '"3.19.0"');

INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'v17-claude-1',
  'claude',
  'V17 Future Claude Relay',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.v17-relay.test/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-v17-unit-tests-only","ANTHROPIC_MODEL":"glm-future"}}',
  'https://v17-relay.test',
  'custom',
  1710000200,
  1,
  'v17 future fixture',
  'anthropic',
  '#D97706',
  '{"apiFormat":"anthropic"}',
  1,
  0
);

INSERT INTO provider_endpoints (provider_id, app_type, url, added_at)
VALUES ('v17-claude-1', 'claude', 'https://api.v17-relay.test/v1', 1710000200);
