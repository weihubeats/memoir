-- 0002: 全文检索（FTS5 trigram，支持中文子串）+ 向量表
-- entries_fts 为 external content 表，靠触发器与 entries 保持同步。

CREATE VIRTUAL TABLE entries_fts USING fts5(
    title, body,
    content = 'entries',
    content_rowid = 'id',
    tokenize = 'trigram'
);

CREATE TRIGGER entries_fts_ai AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts (rowid, title, body) VALUES (new.id, new.title, new.body);
END;

CREATE TRIGGER entries_fts_ad AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts (entries_fts, rowid, title, body)
    VALUES ('delete', old.id, old.title, old.body);
END;

CREATE TRIGGER entries_fts_au AFTER UPDATE ON entries BEGIN
    INSERT INTO entries_fts (entries_fts, rowid, title, body)
    VALUES ('delete', old.id, old.title, old.body);
    INSERT INTO entries_fts (rowid, title, body) VALUES (new.id, new.title, new.body);
END;

INSERT INTO entries_fts (entries_fts) VALUES ('rebuild');

-- 向量：个人库量级（千条内）直接全量载入做余弦，不引 sqlite-vec 扩展
CREATE TABLE embeddings (
    entry_id INTEGER PRIMARY KEY REFERENCES entries(id) ON DELETE CASCADE,
    model    TEXT NOT NULL,
    dim      INTEGER NOT NULL,
    vector   BLOB NOT NULL
);
