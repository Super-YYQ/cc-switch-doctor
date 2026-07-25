PRAGMA user_version = 16;

-- Destructive: settings_config missing → provider_scan Disabled.
CREATE TABLE providers (
    id TEXT NOT NULL,
    app_type TEXT NOT NULL,
    name TEXT NOT NULL,
    meta TEXT NOT NULL DEFAULT '{}',
    is_current BOOLEAN NOT NULL DEFAULT 0,
    PRIMARY KEY (id, app_type)
);

CREATE TABLE provider_endpoints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    app_type TEXT NOT NULL,
    url TEXT NOT NULL,
    added_at INTEGER
);

INSERT INTO providers (id, app_type, name, meta, is_current)
VALUES ('broken-1', 'claude', 'Broken No Settings', '{}', 1);
