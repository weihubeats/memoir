<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useLocaleStore } from './stores/locale'
import { useUiStore, type Tab } from './stores/ui'
import { useWorkspaceStore } from './stores/workspace'
import ProfileView from './views/ProfileView.vue'
import EntityDetailView from './views/EntityDetailView.vue'
import TimelineView from './views/TimelineView.vue'
import ChatView from './views/ChatView.vue'
import ReviewView from './views/ReviewView.vue'
import TrashView from './views/TrashView.vue'
import SettingsView from './views/SettingsView.vue'

const t = useLocaleStore().t
const locale = useLocaleStore()
const ui = useUiStore()
const ws = useWorkspaceStore()

const theme = ref<'dark' | 'light'>(
  (localStorage.getItem('mc-theme') as 'dark' | 'light' | null) ?? 'dark',
)
const searchQuery = ref('')
const searchInput = ref<HTMLInputElement | null>(null)

const tabs: { key: Tab; label: string }[] = [
  { key: 'profile', label: t('nav.profile') },
  { key: 'timeline', label: t('nav.timeline') },
  { key: 'chat', label: t('nav.chat') },
  { key: 'review', label: t('nav.review') },
  { key: 'trash', label: t('nav.trash') },
  { key: 'settings', label: t('nav.settings') },
]

const views = {
  profile: ProfileView,
  entity: EntityDetailView,
  timeline: TimelineView,
  chat: ChatView,
  review: ReviewView,
  trash: TrashView,
  settings: SettingsView,
}

function applyTheme() {
  document.documentElement.dataset.theme = theme.value
  localStorage.setItem('mc-theme', theme.value)
}
function toggleTheme() {
  theme.value = theme.value === 'dark' ? 'light' : 'dark'
  applyTheme()
}
function toggleLocale() {
  locale.setMode(locale.mode === 'zh' ? 'en' : 'zh')
}
function runGlobalSearch() {
  const q = searchQuery.value.trim()
  if (!q) return
  ui.globalSearch(q)
}

function onKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    searchInput.value?.focus()
    searchInput.value?.select()
  }
}

onMounted(() => {
  applyTheme()
  ws.loadAll()
  window.addEventListener('keydown', onKeydown)
})
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="flex h-screen overflow-hidden">
    <aside
      class="flex w-48 shrink-0 flex-col border-r"
      style="border-color: var(--border); background: var(--bg-panel)"
    >
      <div class="px-4 py-4 text-sm font-semibold">{{ t('app.name') }}</div>
      <nav class="flex flex-col gap-1 px-2">
        <button
          v-for="item in tabs"
          :key="item.key"
          class="rounded-md px-3 py-2 text-left text-sm"
          :style="
            ui.tab === item.key
              ? 'background: var(--bg-hover); color: var(--accent)'
              : 'color: var(--text)'
          "
          @click="ui.tab = item.key"
        >
          {{ item.label }}
        </button>
      </nav>
      <div class="mt-auto flex flex-col gap-2 px-3 pb-4">
        <button class="mc-btn text-xs" @click="toggleLocale">
          {{ locale.mode === 'zh' ? 'English' : '中文' }}
        </button>
        <button class="mc-btn text-xs" data-test="theme-toggle" @click="toggleTheme">
          {{ theme === 'dark' ? '☀ Light' : '☾ Dark' }}
        </button>
      </div>
    </aside>
    <div class="flex flex-1 flex-col overflow-hidden">
      <div class="px-6 pt-5">
        <input
          ref="searchInput"
          v-model="searchQuery"
          class="mc-input w-full text-sm"
          :placeholder="t('app.searchPlaceholder')"
          data-test="global-search"
          @keyup.enter="runGlobalSearch"
        />
      </div>
      <main class="flex-1 overflow-y-auto p-6 pt-4">
        <component :is="views[ui.tab]" />
      </main>
    </div>
  </div>
</template>
