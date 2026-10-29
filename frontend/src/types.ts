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
