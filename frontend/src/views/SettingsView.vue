<script setup lang="ts">
import { ref } from 'vue'
import { useLocaleStore } from '../stores/locale'
import { useWorkspaceStore } from '../stores/workspace'

const t = useLocaleStore().t
const ws = useWorkspaceStore()

const format = ref<'markdown' | 'json'>('markdown')
const exported = ref('')
const exporting = ref(false)

const previewQuery = ref('')
const preview = ref('')
const previewing = ref(false)

const mcpSnippet = `{
  "mcpServers": {
    "my-context": { "command": "/path/to/my-context", "args": ["serve"] }
  }
}`

const copiedKey = ref('')
async function copy(key: string, text: string) {
  await navigator.clipboard.writeText(text)
  copiedKey.value = key
  setTimeout(() => {
    if (copiedKey.value === key) copiedKey.value = ''
  }, 1500)
}

async function runExport() {
  exporting.value = true
  try {
    exported.value = await ws.exportData(format.value)
  } finally {
    exporting.value = false
  }
}

async function runPreview() {
  previewing.value = true
  try {
    preview.value = await ws.contextPreview(previewQuery.value || undefined)
  } finally {
    previewing.value = false
  }
}
</script>

<template>
  <section class="mx-auto max-w-3xl">
    <h1 class="mb-4 text-base font-semibold">{{ t('settings.title') }}</h1>

    <div class="mc-panel mb-5 p-4">
      <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('settings.counts') }}</h2>
      <div v-if="ws.info" class="flex gap-8 text-sm">
        <div>
          <div class="text-lg font-semibold">{{ ws.info.entities }}</div>
          <div class="mc-dim text-xs">{{ t('settings.countEntities') }}</div>
        </div>
        <div>
          <div class="text-lg font-semibold">{{ ws.info.facts }}</div>
          <div class="mc-dim text-xs">{{ t('settings.countFacts') }}</div>
        </div>
        <div>
          <div class="text-lg font-semibold">{{ ws.info.entries }}</div>
          <div class="mc-dim text-xs">{{ t('settings.countEntries') }}</div>
        </div>
      </div>
      <div class="mc-dim mt-3 text-xs">
        {{ t('settings.dbPath') }}: <span data-test="db-path">{{ ws.info?.path ?? '…' }}</span>
      </div>
    </div>

    <div class="mc-panel mb-5 p-4">
      <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('settings.mcpTitle') }}</h2>
      <p class="mc-dim mb-2 text-xs">{{ t('settings.mcpHint') }}</p>
      <div class="flex items-start gap-2">
        <pre class="mc-panel flex-1 overflow-auto p-3 text-xs" style="background: var(--bg)">{{ mcpSnippet }}</pre>
        <button class="mc-btn text-xs" @click="copy('mcp', mcpSnippet)">
          {{ copiedKey === 'mcp' ? t('common.copied') : t('common.copy') }}
        </button>
      </div>
    </div>

    <div class="mc-panel mb-5 p-4">
      <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('settings.exportTitle') }}</h2>
      <div class="mb-2 flex items-center gap-2">
        <select v-model="format" class="mc-select text-xs">
          <option value="markdown">markdown</option>
          <option value="json">json</option>
        </select>
        <button class="mc-btn text-xs" :disabled="exporting" @click="runExport">
          {{ t('settings.exportRun') }}
        </button>
        <button
          v-if="exported"
          class="mc-btn text-xs"
          @click="copy('export', exported)"
        >
          {{ copiedKey === 'export' ? t('common.copied') : t('common.copy') }}
        </button>
      </div>
      <textarea
        v-if="exported"
        readonly
        class="mc-textarea h-40 w-full text-xs"
        :value="exported"
      ></textarea>
    </div>

    <div class="mc-panel p-4">
      <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('settings.contextTitle') }}</h2>
      <div class="mb-2 flex items-center gap-2">
        <input v-model="previewQuery" class="mc-input flex-1 text-xs" />
        <button class="mc-btn text-xs" :disabled="previewing" @click="runPreview">
          {{ t('settings.contextRun') }}
        </button>
      </div>
      <pre
        v-if="preview"
        class="mc-panel max-h-72 overflow-auto p-3 text-xs leading-5"
        style="background: var(--bg); white-space: pre-wrap"
        >{{ preview }}</pre
      >
    </div>
  </section>
</template>
