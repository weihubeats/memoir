import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type {
  AuditRow,
  ChatMessage,
  ChatOp,
  Conversation,
  DbInfo,
  Entity,
  Entry,
  Fact,
  StaleFact,
  TrashData,
} from '../types'

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
    trash: null as TrashData | null,
    entityFacts: [] as Fact[],
    conversations: [] as Conversation[],
    messages: [] as ChatMessage[],
    activeConversationId: null as number | null,
    settings: {} as Record<string, string>,
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
    async loadTrash() {
      this.trash = await invoke<TrashData>('list_trash')
    },
    async loadEntityFacts(id: number) {
      this.entityFacts = await invoke<Fact[]>('list_facts', { entityId: id, activeOnly: false })
    },
    async loadInfo() {
      this.info = await invoke<DbInfo>('db_info')
    },
    async deleteEntry(id: number) {
      await invoke('delete_entry', { id })
      await Promise.all([this.reloadEntries(), this.loadTrash(), this.loadInfo()])
    },
    async restoreEntry(id: number) {
      await invoke('restore_entry', { id })
      await Promise.all([this.reloadEntries(), this.loadTrash()])
    },
    async purgeEntry(id: number) {
      await invoke('purge_entry', { id })
      await Promise.all([this.reloadEntries(), this.loadTrash()])
    },
    async deleteFact(id: number) {
      await invoke('delete_fact', { id })
      await Promise.all([this.reloadFacts(), this.loadTrash(), this.reloadStale()])
    },
    async restoreFact(id: number) {
      await invoke('restore_fact', { id })
      await Promise.all([this.reloadFacts(), this.loadTrash(), this.reloadStale()])
    },
    async purgeFact(id: number) {
      await invoke('purge_fact', { id })
      await Promise.all([this.reloadFacts(), this.loadTrash()])
    },
    async deleteEntity(id: number) {
      await invoke('delete_entity', { id })
      await Promise.all([this.reloadEntities(), this.reloadFacts(), this.loadTrash()])
    },
    async restoreEntity(id: number) {
      await invoke('restore_entity', { id })
      await Promise.all([this.reloadEntities(), this.loadTrash()])
    },
    async purgeEntity(id: number) {
      await invoke('purge_entity', { id })
      await this.loadTrash()
    },
    async mergeEntities(sourceId: number, targetId: number) {
      await invoke('merge_entities', { sourceId, targetId })
      await Promise.all([this.reloadEntities(), this.reloadFacts(), this.reloadEntries()])
    },

    // ---------- 内置对话（M4） ----------
    async loadConversations() {
      this.conversations = await invoke<Conversation[]>('chat_list')
    },
    async openConversation(id: number) {
      this.activeConversationId = id
      this.messages = await invoke<ChatMessage[]>('chat_messages', { conversationId: id })
    },
    async newConversation(backend: 'cloud' | 'local') {
      const c = await invoke<Conversation>('chat_new', { backend })
      await this.loadConversations()
      this.activeConversationId = c.id
      this.messages = []
    },
    async deleteConversation(id: number) {
      await invoke('chat_delete', { id })
      if (this.activeConversationId === id) {
        this.activeConversationId = null
        this.messages = []
      }
      await this.loadConversations()
    },
    async sendMessage(content: string) {
      if (!this.activeConversationId) return
      await invoke('chat_send', { args: { conversationId: this.activeConversationId, content } })
    },
    async applyOp(messageId: number, opIndex: number, op: ChatOp): Promise<string> {
      return invoke<string>('apply_chat_op', { args: { messageId, opIndex, op } })
    },
    async loadSettings() {
      this.settings = await invoke<Record<string, string>>('settings_all')
    },
    async setSetting(key: string, value: string) {
      await invoke('settings_set', { key, value })
      this.settings[key] = value
    },
  },
})
