-- 0003: 回收站——三张表加 deleted_at（软删标记，30 天后自动物理清除）

ALTER TABLE entries ADD COLUMN deleted_at TEXT;
ALTER TABLE facts ADD COLUMN deleted_at TEXT;
ALTER TABLE entities ADD COLUMN deleted_at TEXT;

CREATE INDEX idx_entries_deleted ON entries (deleted_at);
CREATE INDEX idx_facts_deleted   ON facts (deleted_at);
CREATE INDEX idx_entities_deleted ON entities (deleted_at);
