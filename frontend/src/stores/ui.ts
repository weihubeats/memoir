import { defineStore } from 'pinia'

export type Tab = 'profile' | 'entity' | 'timeline' | 'review' | 'trash' | 'settings'

export const useUiStore = defineStore('ui', {
  state: () => ({
    tab: 'profile' as Tab,
    selectedEntityId: null as number | null,
    pendingSearch: '',
  }),
  actions: {
    openEntity(id: number) {
      this.selectedEntityId = id
      this.tab = 'entity'
    },
    globalSearch(query: string) {
      this.pendingSearch = query
      this.tab = 'timeline'
    },
  },
})
