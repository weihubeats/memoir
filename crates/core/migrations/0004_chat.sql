-- 0004: 内置对话——会话、消息、写操作采纳记录、应用设置

CREATE TABLE conversations (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    title      TEXT NOT NULL DEFAULT '新对话',
    backend    TEXT NOT NULL DEFAULT 'cloud', -- cloud / local
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content         TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_messages_conversation ON messages (conversation_id, id);

-- 用户对 AI 写操作的采纳记录（撤销 = 无记录，不落库）
CREATE TABLE chat_op_adoptions (
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    op_index   INTEGER NOT NULL,
    adopted_at TEXT NOT NULL,
    PRIMARY KEY (message_id, op_index)
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
