PRAGMA user_version = 16;

-- Provider core intact; proxy_config uses unknown/broken shape → routing disabled only.
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

-- Unknown routing structure (missing critical columns).
CREATE TABLE proxy_config (
    id INTEGER PRIMARY KEY,
    blob TEXT
);

INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'route-unknown-claude',
  'claude',
  'Routing Unknown Claude',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.route-unknown.test/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-route-unknown","ANTHROPIC_MODEL":"glm-route"}}',
  'https://route-unknown.test',
  'custom',
  1710000500,
  1,
  'fixture',
  'anthropic',
  '#D97706',
  '{"apiFormat":"anthropic"}',
  1,
  0
);

INSERT INTO provider_endpoints (provider_id, app_type, url, added_at)
VALUES ('route-unknown-claude', 'claude', 'https://api.route-unknown.test/v1', 1710000500);
