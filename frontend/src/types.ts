export type Sensitivity = 'normal' | 'sensitive' | 'secret'

export interface Entity {
  id: number
  name: string
  kind: string
  domain: string | null
  status: string
  created_at: string
  updated_at: string
  deleted_at: string | null
}

export interface Fact {
  id: number
  entity_id: number
  key: string
  value: string
  domain: string | null
  sensitivity: Sensitivity
  valid_from: string
  valid_to: string | null
  source_entry_id: number | null
  deleted_at: string | null
}

export interface Entry {
  id: number
  occurred_at: string
  kind: string
  title: string
  body: string
  tags: string[]
  entity_ids: number[]
  created_at: string
  deleted_at: string | null
}

export interface AuditRow {
  id: number
  at: string
  action: string
  object_type: string
  object_id: number | null
  source: string
  summary: string
}

export interface StaleFact {
  entity: string
  key: string
  value: string
  updated_at: string
}

export interface DbInfo {
  path: string
  entities: number
  facts: number
  entries: number
}

export interface TrashData {
  entities: Entity[]
  facts: Fact[]
  entries: Entry[]
}

// ---------- 内置对话（M4） ----------

export interface Conversation {
  id: number
  title: string
  backend: 'cloud' | 'local'
  created_at: string
  updated_at: string
}

export type ChatOp =
  | {
      type: 'upsert_fact'
      entity: string
      key: string
      value: unknown
      domain: string | null
      sensitivity: string | null
    }
  | {
      type: 'add_entry'
      title: string
      body: string
      kind: string | null
      tags: string[]
      entities: string[]
    }

export type Segment =
  | { kind: 'text'; content: string }
  | { kind: 'op'; index: number; op: ChatOp }

export interface ChatMessage {
  id: number
  conversation_id: number
  role: 'user' | 'assistant' | 'system'
  content: string
  created_at: string
  segments: Segment[]
  adopted: number[]
}
