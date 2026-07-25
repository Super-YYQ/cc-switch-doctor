PRAGMA user_version = 16;

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

-- Third-party Claude (testable)
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'glm-claude-1',
  'claude',
  'GLM Relay',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.example-relay.test/v1","ANTHROPIC_AUTH_TOKEN":"sk-test-fake-key-for-unit-tests-only-claude","ANTHROPIC_MODEL":"glm-4.5"}}',
  'https://example-relay.test',
  'custom',
  1710000000,
  1,
  'fixture',
  'anthropic',
  '#D97706',
  '{"apiFormat":"anthropic"}',
  1,
  0
);

-- Third-party Codex (testable)
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'minimax-codex-1',
  'codex',
  'MiniMax Codex',
  '{"auth":{"OPENAI_API_KEY":"sk-test-fake-key-for-unit-tests-only-codex"},"config":"model_provider = \"minimax\"\nmodel = \"MiniMax-M2.5\"\n\n[model_providers.minimax]\nname = \"MiniMax\"\nbase_url = \"https://api.minimax-relay.test/v1\"\nwire_api = \"chat\"\n"}',
  'https://minimax-relay.test',
  'custom',
  1710000001,
  1,
  'fixture',
  'openai',
  '#10B981',
  '{}',
  1,
  0
);

-- Managed OAuth (must be skipped)
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'codex-official-oauth',
  'codex',
  'Codex Official OAuth',
  '{"auth":{},"config":"model_provider = \"openai\"\n"}',
  'https://chatgpt.com',
  'official',
  1710000002,
  0,
  'fixture oauth',
  'openai',
  '#000000',
  '{"providerType":"codex_oauth"}',
  0,
  0
);

-- GitHub Copilot managed
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'gh-copilot-1',
  'claude',
  'GitHub Copilot',
  '{"env":{"ANTHROPIC_BASE_URL":"https://api.githubcopilot.com","ANTHROPIC_AUTH_TOKEN":""}}',
  'https://github.com',
  'official',
  1710000003,
  2,
  'fixture copilot',
  'github',
  '#333333',
  '{"providerType":"github_copilot"}',
  0,
  0
);

-- OpenCode third-party
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'opencode-openai-1',
  'opencode',
  'OpenCode OpenAI Relay',
  '{"options":{"baseURL":"https://api.openai-compat.test/v1","apiKey":"sk-test-fake-key-for-unit-tests-only-opencode"},"npm":"@ai-sdk/openai"}',
  'https://openai-compat.test',
  'custom',
  1710000004,
  1,
  'fixture',
  'openai',
  '#0EA5E9',
  '{}',
  1,
  0
);

-- Gemini third-party
INSERT INTO providers (id, app_type, name, settings_config, website_url, category, created_at, sort_index, notes, icon, icon_color, meta, is_current, in_failover_queue)
VALUES (
  'gemini-relay-1',
  'gemini',
  'Gemini Relay',
  '{"env":{"GOOGLE_GEMINI_BASE_URL":"https://generativelanguage.example.test","GEMINI_API_KEY":"sk-test-fake-key-for-unit-tests-only-gemini","GEMINI_MODEL":"gemini-2.0-flash"}}',
  'https://generativelanguage.example.test',
  'custom',
  1710000005,
  1,
  'fixture',
  'gemini',
  '#4285F4',
  '{}',
  1,
  0
);

INSERT INTO provider_endpoints (provider_id, app_type, url, added_at)
VALUES ('minimax-codex-1', 'codex', 'https://api.minimax-relay.test', 1710000100);

INSERT INTO settings (key, value) VALUES ('app_version', '"3.17.0"');
