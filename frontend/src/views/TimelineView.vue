<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useLocaleStore } from '../stores/locale'
import { useUiStore } from '../stores/ui'
import { useWorkspaceStore, type AddEntryPayload } from '../stores/workspace'
import type { Entry } from '../types'

const t = useLocaleStore().t
const ws = useWorkspaceStore()
const ui = useUiStore()

const query = ref('')
const results = ref<Entry[] | null>(null)
const searching = ref(false)

const showForm = ref(false)
const submitting = ref(false)
const form = ref<AddEntryPayload>({ title: '', body: '', kind: 'note', tags: [], entities: [] })

const showImport = ref(false)
const importing = ref(false)
const importForm = ref({ title: '', body: '', entities: '' })

async function runImport() {
  if (!importForm.value.title || !importForm.value.body) return
  importing.value = true
  try {
    await ws.addEntry({
      title: importForm.value.title,
      body: importForm.value.body,
      kind: 'import',
      tags: [],
      entities: importForm.value.entities
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean),
    })
    importForm.value = { title: '', body: '', entities: '' }
    showImport.value = false
  } finally {
    importing.value = false
  }
}

watch(
  () => ui.pendingSearch,
  (q) => {
    if (q) {
      query.value = q
      results.value = null
      runSearch()
      ui.pendingSearch = ''
    }
  },
)

async function runSearch() {
  const q = query.value.trim()
  if (!q) return
  searching.value = true
  try {
    results.value = await ws.searchEntries(q)
  } finally {
    searching.value = false
  }
}
function backToAll() {
  query.value = ''
  results.value = null
}

async function submit() {
  if (!form.value.title) return
  submitting.value = true
  try {
    await ws.addEntry({
      ...form.value,
      entities: form.value.entities.map((s) => s.trim()).filter(Boolean),
    })
    form.value = { title: '', body: '', kind: 'note', tags: [], entities: [] }
    showForm.value = false
  } finally {
    submitting.value = false
  }
}

const shown = (): Entry[] => results.value ?? ws.entries

onMounted(() => {
  if (ws.entries.length === 0) ws.reloadEntries()
})
</script>

<template>
  <section class="mx-auto max-w-3xl">
    <div class="mb-4 flex items-center justify-between">
      <h1 class="text-base font-semibold">{{ t('timeline.title') }}</h1>
      <div class="flex gap-2">
        <button class="mc-btn text-xs" @click="showImport = !showImport">
          {{ t('timeline.import') }}
        </button>
        <button class="mc-btn mc-btn-primary text-xs" @click="showForm = !showForm">
          {{ t('timeline.newEntry') }}
        </button>
      </div>
    </div>

    <div class="mb-4 flex gap-2">
      <input
        v-model="query"
        class="mc-input flex-1"
        :placeholder="t('timeline.filterPlaceholder')"
        data-test="search-input"
        @keyup.enter="runSearch"
      />
      <button class="mc-btn text-xs" :disabled="searching" @click="runSearch">
        {{ t('timeline.search') }}
      </button>
      <button v-if="results !== null" class="mc-btn text-xs" @click="backToAll">
        {{ t('timeline.backToAll') }}
      </button>
    </div>

    <form v-if="showForm" class="mc-panel mb-4 flex flex-col gap-3 p-4" @submit.prevent="submit">
      <label class="flex flex-col gap-1 text-xs">
        {{ t('timeline.entryTitle') }}
        <input v-model="form.title" class="mc-input" required />
      </label>
      <label class="flex flex-col gap-1 text-xs">
        {{ t('timeline.body') }}
        <textarea v-model="form.body" class="mc-textarea" rows="4"></textarea>
      </label>
      <div class="grid grid-cols-2 gap-3">
        <label class="flex flex-col gap-1 text-xs">
          {{ t('common.kind') }}
          <select v-model="form.kind" class="mc-select">
            <option value="note">note</option>
            <option value="diary">diary</option>
            <option value="event">event</option>
            <option value="chat">chat</option>
          </select>
        </label>
        <label class="flex flex-col gap-1 text-xs">
          {{ t('timeline.entities') }}
          <input v-model="form.entities" class="mc-input" placeholder="我, 父亲" />
        </label>
      </div>
      <div class="text-right">
        <button class="mc-btn mc-btn-primary text-xs" type="submit" :disabled="submitting">
          {{ t('common.submit') }}
        </button>
      </div>
    </form>

    <form v-if="showImport" class="mc-panel mb-4 flex flex-col gap-3 p-4" @submit.prevent="runImport">
      <label class="flex flex-col gap-1 text-xs">
        {{ t('timeline.importTitle') }}
        <input v-model="importForm.title" class="mc-input" required />
      </label>
      <label class="flex flex-col gap-1 text-xs">
        {{ t('timeline.importBody') }}
        <textarea v-model="importForm.body" class="mc-textarea h-40" required></textarea>
      </label>
      <label class="flex flex-col gap-1 text-xs">
        {{ t('timeline.entities') }}
        <input v-model="importForm.entities" class="mc-input" placeholder="我" />
      </label>
      <div class="text-right">
        <button class="mc-btn mc-btn-primary text-xs" type="submit" :disabled="importing">
          {{ t('common.submit') }}
        </button>
      </div>
    </form>

    <div class="flex flex-col gap-3">
      <article v-for="e in shown()" :key="e.id" class="mc-panel p-4" data-test="entry-item">
        <div class="mb-1 flex items-baseline gap-2">
          <span class="mc-dim text-xs">#{{ e.id }}</span>
          <span class="mc-badge">{{ e.kind }}</span>
          <span class="text-sm font-medium">{{ e.title }}</span>
          <span class="mc-dim ml-auto text-xs">{{ e.occurred_at }}</span>
        </div>
        <p v-if="e.body" class="whitespace-pre-wrap text-sm">{{ e.body }}</p>
        <div v-if="e.entity_ids.length" class="mt-2 flex flex-wrap gap-1">
          <span v-for="id in e.entity_ids" :key="id" class="mc-badge">{{ ws.entityName(id) }}</span>
        </div>
      </article>
      <p v-if="shown().length === 0" class="mc-dim text-sm">{{ t('common.empty') }}</p>
    </div>
  </section>
</template>
