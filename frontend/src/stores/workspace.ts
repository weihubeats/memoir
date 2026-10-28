import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { AuditRow, DbInfo, Entity, Entry, Fact, StaleFact } from '../types'

export interface UpsertFactPayload {
  entity: string
  key: string
  value: string
  domain: string | null
  sensitivity: string
}

export interface AddEntryPayload {
  title: string
  body: string
  kind: string
  tags: string[]
  entities: string[]
}

function parseValue(raw: string): string {
  try {
    const v: unknown = JSON.parse(raw)
    return typeof v === 'string' ? v : JSON.stringify(v)
  } catch {
    return raw
  }
}

export const useWorkspaceStore = defineStore('workspace', {
  state: () => ({
    info: null as DbInfo | null,
    entities: [] as Entity[],
    facts: [] as Fact[],
    entries: [] as Entry[],
    audit: [] as AuditRow[],
    stale: [] as StaleFact[],
    loading: false,
    error: '',
  }),
  getters: {
    displayValue: () => (raw: string) => parseValue(raw),
    domainGroups(state): { domain: string; facts: Fact[] }[] {
      const groups = new Map<string, Fact[]>()
      for (const f of state.facts) {
        const d = f.domain ?? 'general'
        if (!groups.has(d)) groups.set(d, [])
        groups.get(d)!.push(f)
      }
      return [...groups.entries()].map(([domain, facts]) => ({ domain, facts }))
    },
    entityName(state) {
      return (id: number) => state.entities.find((e) => e.id === id)?.name ?? `#${id}`
    },
  },
  actions: {
    async loadAll() {
      this.loading = true
      this.error = ''
      try {
        const [info, entities, facts, entries, audit, stale] = await Promise.all([
          invoke<DbInfo>('db_info'),
          invoke<Entity[]>('list_entities'),
          invoke<Fact[]>('list_facts', { activeOnly: true }),
          invoke<Entry[]>('list_entries', { limit: 100 }),
          invoke<AuditRow[]>('list_audit', { limit: 50 }),
          invoke<StaleFact[]>('stale_facts'),
        ])
        this.info = info
        this.entities = entities
        this.facts = facts
        this.entries = entries
        this.audit = audit
        this.stale = stale
      } catch (e) {
        this.error = String(e)
      } finally {
        this.loading = false
      }
    },
    async searchEntries(query: string): Promise<Entry[]> {
      return invoke<Entry[]>('search_entries', { query })
    },
    async upsertFact(p: UpsertFactPayload) {
      await invoke('upsert_fact', { args: p })
      await Promise.all([this.reloadFacts(), this.reloadStale()])
    },
    async addEntry(p: AddEntryPayload) {
      await invoke('add_entry', { args: p })
      await this.reloadEntries()
    },
    async addEntity(name: string, kind: string, domain: string | null) {
      await invoke('add_entity', { name, kind, domain })
      await this.reloadEntities()
    },
    async reloadFacts() {
      this.facts = await invoke<Fact[]>('list_facts', { activeOnly: true })
    },
    async reloadEntities() {
      this.entities = await invoke<Entity[]>('list_entities')
    },
    async reloadStale() {
      this.stale = await invoke<StaleFact[]>('stale_facts')
    },
    async reloadEntries() {
      this.entries = await invoke<Entry[]>('list_entries', { limit: 100 })
    },
    async exportData(format: string): Promise<string> {
      return invoke<string>('export_data', { format })
    },
    async monthlyReport(year?: number, month?: number): Promise<string> {
      return invoke<string>('monthly_report', { year, month })
    },
    async contextPreview(query?: string): Promise<string> {
      return invoke<string>('get_context_preview', { query })
    },
  },
})
