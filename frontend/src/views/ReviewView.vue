<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useLocaleStore } from '../stores/locale'
import { useWorkspaceStore } from '../stores/workspace'

const t = useLocaleStore().t
const ws = useWorkspaceStore()

const report = ref('')
const generating = ref(false)

async function generate() {
  generating.value = true
  try {
    report.value = await ws.monthlyReport()
  } finally {
    generating.value = false
  }
}

onMounted(() => {
  if (ws.stale.length === 0) ws.reloadStale()
})
</script>

<template>
  <section class="mx-auto max-w-3xl">
    <h1 class="mb-4 text-base font-semibold">{{ t('review.title') }}</h1>

    <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('review.staleTitle') }}</h2>
    <div v-if="ws.stale.length" class="mc-panel mb-6 divide-y" style="border-color: var(--border)">
      <div
        v-for="(s, i) in ws.stale"
        :key="i"
        class="flex items-center gap-3 px-4 py-2.5 text-sm"
        :style="{ borderColor: 'var(--border)' }"
        data-test="stale-row"
      >
        <span class="font-medium">{{ s.entity }}</span>
        <span class="mc-dim">·</span>
        <span>{{ s.key }}</span>
        <span class="mc-dim">=</span>
        <span class="flex-1 truncate">{{ ws.displayValue(s.value) }}</span>
        <span class="mc-dim text-xs">{{ s.updated_at }}</span>
      </div>
    </div>
    <p v-else class="mc-dim mb-6 text-sm">{{ t('review.staleEmpty') }}</p>

    <div class="mb-6">
      <div class="mb-2 flex items-center justify-between">
        <h2 class="mc-dim text-xs font-semibold uppercase">{{ t('review.reportTitle') }}</h2>
        <button class="mc-btn text-xs" :disabled="generating" @click="generate">
          {{ t('review.generate') }}
        </button>
      </div>
      <pre
        v-if="report"
        class="mc-panel max-h-96 overflow-auto p-4 text-xs leading-5"
        style="white-space: pre-wrap"
        >{{ report }}</pre
      >
    </div>

    <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('review.auditTitle') }}</h2>
    <div class="mc-panel divide-y" style="border-color: var(--border)">
      <div
        v-for="a in ws.audit"
        :key="a.id"
        class="flex items-center gap-3 px-4 py-2 text-xs"
        :style="{ borderColor: 'var(--border)' }"
        data-test="audit-row"
      >
        <span class="mc-dim w-40 shrink-0">{{ a.at }}</span>
        <span class="w-24 shrink-0 font-medium">{{ a.action }}</span>
        <span class="mc-badge w-16 shrink-0 text-center">{{ a.source }}</span>
        <span class="truncate">{{ a.summary }}</span>
      </div>
    </div>
  </section>
</template>
