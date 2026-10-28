<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useLocaleStore } from './stores/locale'
import { useWorkspaceStore } from './stores/workspace'
import ProfileView from './views/ProfileView.vue'
import TimelineView from './views/TimelineView.vue'
import ReviewView from './views/ReviewView.vue'
import SettingsView from './views/SettingsView.vue'

type Tab = 'profile' | 'timeline' | 'review' | 'settings'

const t = useLocaleStore().t
const locale = useLocaleStore()
const ws = useWorkspaceStore()

const tab = ref<Tab>('profile')
const theme = ref<'dark' | 'light'>(
  (localStorage.getItem('mc-theme') as 'dark' | 'light' | null) ?? 'dark',
)

const tabs: { key: Tab; label: ReturnType<typeof t> }[] = [
  { key: 'profile', label: t('nav.profile') },
  { key: 'timeline', label: t('nav.timeline') },
  { key: 'review', label: t('nav.review') },
  { key: 'settings', label: t('nav.settings') },
]

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

onMounted(() => {
  applyTheme()
  ws.loadAll()
})
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
            tab === item.key
              ? 'background: var(--bg-hover); color: var(--accent)'
              : 'color: var(--text)'
          "
          @click="tab = item.key"
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
    <main class="flex-1 overflow-y-auto p-6">
      <component :is="{ profile: ProfileView, timeline: TimelineView, review: ReviewView, settings: SettingsView }[tab]" />
    </main>
  </div>
</template>
