PRAGMA user_version = 13;

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

-- Third-party Claude (testable on v13)
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'v13-claude-1',
  'claude',
  'V13 Claude Relay',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.v13-relay.test/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-v13-unit-tests-only","ANTHROPIC_MODEL":"claude-3-5-sonnet"}}',
  'https://v13-relay.test',
  'custom',
  1710000100,
  1,
  'v13 fixture',
  'anthropic',
  '#D97706',
  '{"apiFormat":"anthropic"}',
  1,
  0
);

INSERT INTO provider_endpoints (provider_id, app_type, url, added_at)
VALUES ('v13-claude-1', 'claude', 'https://api.v13-relay.test/v1', 1710000100);

-- Codex third-party
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'v13-codex-1',
  'codex',
  'V13 Codex Relay',
  '{"auth":{"OPENAI_API_KEY":"sk-test-fake-key-for-v13-codex-only"},"config":"model_provider = \"relay\"\nmodel = \"gpt-test\"\n\n[model_providers.relay]\nname = \"Relay\"\nbase_url = \"https://api.v13-codex.test/v1\"\nwire_api = \"chat\"\n"}',
  'https://v13-codex.test',
  'custom',
  1710000101,
  2,
  'v13 fixture',
  'openai',
  '#10B981',
  '{}',
  0,
  0
);

-- Managed OAuth must still be skipped
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'v13-codex-oauth',
  'codex',
  'Codex Official OAuth V13',
  '{"auth":{},"config":"model_provider = \"openai\"\n"}',
  'https://chatgpt.com',
  'official',
  1710000102,
  3,
  'managed',
  'openai',
  '#000',
  '{}',
  0,
  0
);
