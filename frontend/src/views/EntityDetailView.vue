<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useLocaleStore } from '../stores/locale'
import { useUiStore } from '../stores/ui'
import { useWorkspaceStore } from '../stores/workspace'

const t = useLocaleStore().t
const ws = useWorkspaceStore()
const ui = useUiStore()

const entity = computed(() => ws.entities.find((e) => e.id === ui.selectedEntityId) ?? null)
const activeFacts = computed(() => ws.entityFacts.filter((f) => f.valid_to === null))
const historyFacts = computed(() => ws.entityFacts.filter((f) => f.valid_to !== null))
const relatedEntries = computed(() =>
  ws.entries.filter((e) => e.entity_ids.includes(ui.selectedEntityId ?? -1)),
)
const otherEntities = computed(() => ws.entities.filter((e) => e.id !== ui.selectedEntityId))

const editing = ref<number | null>(null)
const editForm = ref({ key: '', value: '', sensitivity: 'normal' })
const merging = ref(false)
const mergeTarget = ref<number | null>(null)

function badgeClass(sens: string): string {
  if (sens === 'secret') return 'mc-badge mc-badge-secret'
  if (sens === 'sensitive') return 'mc-badge mc-badge-sensitive'
  return 'mc-badge'
}

function startEdit(f: { id: number; key: string; value: string; sensitivity: string }) {
  editing.value = f.id
  editForm.value = { key: f.key, value: ws.displayValue(f.value), sensitivity: f.sensitivity }
}

async function submitEdit() {
  if (!entity.value || editing.value === null) return
  await ws.upsertFact({
    entity: entity.value.name,
    key: editForm.value.key,
    value: editForm.value.value,
    domain: entity.value.domain,
    sensitivity: editForm.value.sensitivity,
  })
  editing.value = null
  await ws.loadEntityFacts(entity.value.id)
}

async function removeEntity() {
  if (!entity.value) return
  await ws.deleteEntity(entity.value.id)
  ui.tab = 'profile'
}

async function confirmMerge() {
  if (!entity.value || mergeTarget.value === null) return
  await ws.mergeEntities(entity.value.id, mergeTarget.value)
  merging.value = false
  ui.tab = 'profile'
}

onMounted(() => {
  if (ui.selectedEntityId !== null) ws.loadEntityFacts(ui.selectedEntityId)
  if (ws.entities.length === 0) ws.reloadEntities()
})
</script>

<template>
  <section class="mx-auto max-w-3xl">
    <div class="mb-4 flex items-center gap-3">
      <button class="mc-btn text-xs" @click="ui.tab = 'profile'">{{ t('entity.back') }}</button>
      <h1 v-if="entity" class="text-base font-semibold">
        {{ entity.name }}
        <span class="mc-badge ml-1">{{ entity.kind }}</span>
        <span v-if="entity.domain" class="mc-badge ml-1">{{ entity.domain }}</span>
      </h1>
      <div class="ml-auto flex gap-2">
        <button class="mc-btn text-xs" @click="merging = !merging">
          {{ t('entity.merge') }}
        </button>
        <button class="mc-btn text-xs" style="color: #ef4444" @click="removeEntity">
          {{ t('entity.delete') }}
        </button>
      </div>
    </div>

    <p v-if="!entity" class="mc-dim text-sm">{{ t('common.empty') }}</p>

    <template v-else>
      <form v-if="merging" class="mc-panel mb-4 flex items-end gap-3 p-4" @submit.prevent="confirmMerge">
        <label class="flex flex-1 flex-col gap-1 text-xs">
          {{ t('entity.mergeTarget') }}
          <select v-model="mergeTarget" class="mc-select" required>
            <option v-for="e in otherEntities" :key="e.id" :value="e.id">{{ e.name }}</option>
          </select>
        </label>
        <button class="mc-btn mc-btn-primary text-xs" type="submit">
          {{ t('entity.mergeConfirm') }}
        </button>
      </form>

      <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('entity.activeFacts') }}</h2>
      <div class="mc-panel mb-5 divide-y" style="border-color: var(--border)">
        <div
          v-for="f in activeFacts"
          :key="f.id"
          class="px-4 py-2.5 text-sm"
          :style="{ borderColor: 'var(--border)' }"
          data-test="entity-fact"
        >
          <template v-if="editing === f.id">
            <div class="flex flex-wrap items-center gap-2">
              <input v-model="editForm.value" class="mc-input flex-1 text-sm" />
              <select v-model="editForm.sensitivity" class="mc-select text-xs">
                <option value="normal">{{ t('sens.normal') }}</option>
                <option value="sensitive">{{ t('sens.sensitive') }}</option>
                <option value="secret">{{ t('sens.secret') }}</option>
              </select>
              <button class="mc-btn mc-btn-primary text-xs" @click="submitEdit">
                {{ t('common.submit') }}
              </button>
              <button class="mc-btn text-xs" @click="editing = null">
                {{ t('common.close') }}
              </button>
            </div>
          </template>
          <template v-else>
            <div class="flex items-center gap-3">
              <span class="font-medium">{{ f.key }}</span>
              <span class="mc-dim">=</span>
              <span class="flex-1 truncate">{{ ws.displayValue(f.value) }}</span>
              <span :class="badgeClass(f.sensitivity)">{{ t(`sens.${f.sensitivity}`) }}</span>
              <button class="mc-btn text-xs" @click="startEdit(f)">{{ t('entity.edit') }}</button>
              <button class="mc-btn text-xs" style="color: #ef4444" @click="ws.deleteFact(f.id)">
                {{ t('common.delete') }}
              </button>
            </div>
          </template>
        </div>
        <p v-if="activeFacts.length === 0" class="mc-dim px-4 py-3 text-sm">
          {{ t('common.empty') }}
        </p>
      </div>

      <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('entity.history') }}</h2>
      <div v-if="historyFacts.length" class="mc-panel mb-5 divide-y" style="border-color: var(--border)">
        <div
          v-for="f in historyFacts"
          :key="f.id"
          class="flex items-center gap-3 px-4 py-2 text-xs"
          :style="{ borderColor: 'var(--border)' }"
          data-test="history-fact"
        >
          <span>{{ f.key }}</span>
          <span class="mc-dim">=</span>
          <span class="flex-1 truncate">{{ ws.displayValue(f.value) }}</span>
          <span class="mc-dim">
            {{ t('entity.validRange', { from: f.valid_from, to: f.valid_to ?? '' }) }}
          </span>
          <button
            v-if="f.deleted_at"
            class="mc-btn text-xs"
            @click="ws.restoreFact(f.id).then(() => ws.loadEntityFacts(entity!.id))"
          >
            {{ t('common.restore') }}
          </button>
        </div>
      </div>
      <p v-else class="mc-dim mb-5 text-sm">{{ t('entity.noHistory') }}</p>

      <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ t('entity.relatedEntries') }}</h2>
      <div class="flex flex-col gap-2">
        <article v-for="e in relatedEntries" :key="e.id" class="mc-panel p-3 text-sm">
          <div class="flex items-baseline gap-2">
            <span class="mc-dim text-xs">#{{ e.id }}</span>
            <span class="font-medium">{{ e.title }}</span>
            <span class="mc-dim ml-auto text-xs">{{ e.occurred_at }}</span>
          </div>
        </article>
        <p v-if="relatedEntries.length === 0" class="mc-dim text-sm">{{ t('common.empty') }}</p>
      </div>
    </template>
  </section>
</template>
