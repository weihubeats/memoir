<script setup lang="ts">
import { onMounted } from 'vue'
import { useLocaleStore } from '../stores/locale'
import { useWorkspaceStore } from '../stores/workspace'

const t = useLocaleStore().t
const ws = useWorkspaceStore()

onMounted(() => {
  if (!ws.trash) ws.loadTrash()
})
</script>

<template>
  <section class="mx-auto max-w-3xl">
    <div class="mb-4 flex items-center justify-between">
      <h1 class="text-base font-semibold">{{ t('trash.title') }}</h1>
      <div class="flex items-center gap-2">
        <span class="mc-dim text-xs">{{ t('trash.autoPurgeHint') }}</span>
        <button class="mc-btn text-xs" @click="ws.loadTrash()">{{ t('common.refresh') }}</button>
      </div>
    </div>

    <p v-if="ws.error" class="mc-panel mb-4 p-3 text-xs" style="color: #ef4444">
      {{ t('common.error', { msg: ws.error }) }}
    </p>

    <template v-if="ws.trash">
      <p
        v-if="
          ws.trash.entries.length === 0 && ws.trash.facts.length === 0 && ws.trash.entities.length === 0
        "
        class="mc-dim text-sm"
        data-test="trash-empty"
      >
        {{ t('trash.empty') }}
      </p>

      <template v-if="ws.trash.entries.length">
        <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('trash.entries') }}</h2>
        <div class="mc-panel mb-5 divide-y" style="border-color: var(--border)">
          <div
            v-for="e in ws.trash.entries"
            :key="e.id"
            class="flex items-center gap-3 px-4 py-2.5 text-sm"
            :style="{ borderColor: 'var(--border)' }"
          >
            <span class="mc-dim text-xs">#{{ e.id }}</span>
            <span class="flex-1 truncate">{{ e.title }}</span>
            <span class="mc-dim text-xs">{{ e.deleted_at }}</span>
            <button class="mc-btn text-xs" @click="ws.restoreEntry(e.id)">
              {{ t('common.restore') }}
            </button>
            <button class="mc-btn text-xs" style="color: #ef4444" @click="ws.purgeEntry(e.id)">
              {{ t('common.purge') }}
            </button>
          </div>
        </div>
      </template>

      <template v-if="ws.trash.facts.length">
        <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('trash.facts') }}</h2>
        <div class="mc-panel mb-5 divide-y" style="border-color: var(--border)">
          <div
            v-for="f in ws.trash.facts"
            :key="f.id"
            class="flex items-center gap-3 px-4 py-2.5 text-sm"
            :style="{ borderColor: 'var(--border)' }"
          >
            <span class="flex-1 truncate">{{ f.key }} = {{ ws.displayValue(f.value) }}</span>
            <span class="mc-dim text-xs">{{ f.deleted_at }}</span>
            <button class="mc-btn text-xs" @click="ws.restoreFact(f.id)">
              {{ t('common.restore') }}
            </button>
            <button class="mc-btn text-xs" style="color: #ef4444" @click="ws.purgeFact(f.id)">
              {{ t('common.purge') }}
            </button>
          </div>
        </div>
      </template>

      <template v-if="ws.trash.entities.length">
        <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('trash.entities') }}</h2>
        <div class="mc-panel divide-y" style="border-color: var(--border)">
          <div
            v-for="e in ws.trash.entities"
            :key="e.id"
            class="flex items-center gap-3 px-4 py-2.5 text-sm"
            :style="{ borderColor: 'var(--border)' }"
          >
            <span class="font-medium">{{ e.name }}</span>
            <span class="mc-badge">{{ e.kind }}</span>
            <span class="flex-1"></span>
            <button class="mc-btn text-xs" @click="ws.restoreEntity(e.id)">
              {{ t('common.restore') }}
            </button>
            <button class="mc-btn text-xs" style="color: #ef4444" @click="ws.purgeEntity(e.id)">
              {{ t('common.purge') }}
            </button>
          </div>
        </div>
      </template>
    </template>
  </section>
</template>
