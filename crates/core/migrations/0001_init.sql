-- 0001: 初始 schema（应用后不可修改；变更一律追加新迁移）
-- 时间一律存 UTC RFC3339 文本，字典序即时间序。

CREATE TABLE entities (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL DEFAULT 'topic', -- self / person / org / account / asset / topic
    domain     TEXT,                          -- 实体主域（可空；self 跨域）
    status     TEXT NOT NULL DEFAULT 'active',-- active / archived
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (name)
);

CREATE TABLE facts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id       INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    key             TEXT NOT NULL,
    value           TEXT NOT NULL,                 -- JSON 文本
    domain          TEXT,                          -- 事实所属域（可空 → general）
    sensitivity     TEXT NOT NULL DEFAULT 'normal'
                    CHECK (sensitivity IN ('normal', 'sensitive', 'secret')),
    valid_from      TEXT NOT NULL,
    valid_to        TEXT,                          -- NULL = 当前有效
    source_entry_id INTEGER,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX idx_facts_entity_key ON facts (entity_id, key);
CREATE INDEX idx_facts_active     ON facts (valid_to);
CREATE INDEX idx_facts_domain     ON facts (domain);

CREATE TABLE entries (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT 'note', -- note / diary / event / chat / import
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',     -- Markdown
    tags        TEXT NOT NULL DEFAULT '[]',   -- JSON 字符串数组
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_entries_occurred ON entries (occurred_at DESC);

CREATE TABLE entry_entities (
    entry_id  INTEGER NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    PRIMARY KEY (entry_id, entity_id)
);

CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    at          TEXT NOT NULL,
    action      TEXT NOT NULL,                -- add_entity / upsert_fact / add_entry / ...
    object_type TEXT NOT NULL,                -- entity / fact / entry
    object_id   INTEGER,
    source      TEXT NOT NULL,                -- ai / cli / import / system / test
    summary     TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_audit_at ON audit_log (at DESC);
