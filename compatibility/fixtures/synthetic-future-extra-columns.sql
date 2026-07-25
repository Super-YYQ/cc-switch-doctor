PRAGMA user_version = 18;

-- Future schema: unrelated table + unknown provider column; core still intact.
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
    future_field TEXT,
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

CREATE TABLE unrelated_table (
    id INTEGER PRIMARY KEY,
    payload TEXT
);

INSERT INTO unrelated_table (payload) VALUES ('ignored-by-doctor');

INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue, future_field)
VALUES (
  'extra-claude-1',
  'claude',
  'Extra Column Claude',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.extra-relay.test/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-extra-cols-only","ANTHROPIC_MODEL":"glm-extra"}}',
  'https://extra-relay.test',
  'custom',
  1710000300,
  1,
  'extra columns fixture',
  'anthropic',
  '#D97706',
  '{"apiFormat":"anthropic"}',
  1,
  0,
  '{"future":true}'
);

INSERT INTO provider_endpoints (provider_id, app_type, url, added_at)
VALUES ('extra-claude-1', 'claude', 'https://api.extra-relay.test/v1', 1710000300);
