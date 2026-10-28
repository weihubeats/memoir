<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useLocaleStore } from '../stores/locale'
import { useWorkspaceStore } from '../stores/workspace'

const t = useLocaleStore().t
const ws = useWorkspaceStore()

const DOMAINS = ['health', 'wealth', 'career', 'family', 'relationship', 'preference', 'goal']

const showForm = ref(false)
const form = ref({ entity: '', key: '', value: '', domain: '', sensitivity: 'normal' })
const submitting = ref(false)

const entityNames = computed(() => ws.entities.map((e) => e.name))

function badgeClass(sens: string): string {
  if (sens === 'secret') return 'mc-badge mc-badge-secret'
  if (sens === 'sensitive') return 'mc-badge mc-badge-sensitive'
  return 'mc-badge'
}

async function submit() {
  if (!form.value.entity || !form.value.key || !form.value.value) return
  submitting.value = true
  try {
    await ws.upsertFact({
      entity: form.value.entity,
      key: form.value.key,
      value: form.value.value,
      domain: form.value.domain || null,
      sensitivity: form.value.sensitivity,
    })
    form.value = { entity: '', key: '', value: '', domain: '', sensitivity: 'normal' }
    showForm.value = false
  } finally {
    submitting.value = false
  }
}

onMounted(() => {
  if (ws.facts.length === 0) ws.reloadFacts()
  if (ws.entities.length === 0) ws.reloadEntities()
})
</script>

<template>
  <section class="mx-auto max-w-3xl">
    <div class="mb-4 flex items-center justify-between">
      <h1 class="text-base font-semibold">{{ t('profile.title') }}</h1>
      <div class="flex items-center gap-2">
        <span class="mc-dim text-xs">{{ t('profile.entityCount', { count: ws.entities.length }) }}</span>
        <button class="mc-btn text-xs" @click="ws.loadAll()">{{ t('common.refresh') }}</button>
        <button class="mc-btn mc-btn-primary text-xs" @click="showForm = !showForm">
          {{ t('profile.newFact') }}
        </button>
      </div>
    </div>

    <div v-if="ws.error" class="mc-panel mb-4 p-3 text-xs" style="color: #ef4444">
      {{ t('common.error', { msg: ws.error }) }}
    </div>

    <form v-if="showForm" class="mc-panel mb-4 grid grid-cols-2 gap-3 p-4" @submit.prevent="submit">
      <label class="flex flex-col gap-1 text-xs">
        {{ t('common.entity') }}
        <input v-model="form.entity" class="mc-input" list="entity-names" required />
        <datalist id="entity-names">
          <option v-for="n in entityNames" :key="n" :value="n" />
        </datalist>
      </label>
      <label class="flex flex-col gap-1 text-xs">
        {{ t('profile.key') }}
        <input v-model="form.key" class="mc-input" required />
      </label>
      <label class="flex flex-col gap-1 text-xs">
        {{ t('profile.value') }}
        <input v-model="form.value" class="mc-input" required />
      </label>
      <label class="flex flex-col gap-1 text-xs">
        {{ t('common.domain') }}
        <select v-model="form.domain" class="mc-select">
          <option value="">{{ t('common.none') }}</option>
          <option v-for="d in DOMAINS" :key="d" :value="d">{{ d }}</option>
        </select>
      </label>
      <label class="flex flex-col gap-1 text-xs">
        {{ t('common.sensitivity') }}
        <select v-model="form.sensitivity" class="mc-select">
          <option value="normal">{{ t('sens.normal') }}</option>
          <option value="sensitive">{{ t('sens.sensitive') }}</option>
          <option value="secret">{{ t('sens.secret') }}</option>
        </select>
      </label>
      <div class="col-span-2 text-right">
        <button class="mc-btn mc-btn-primary text-xs" type="submit" :disabled="submitting">
          {{ t('common.submit') }}
        </button>
      </div>
    </form>

    <div v-for="group in ws.domainGroups" :key="group.domain" class="mb-5">
      <h2 class="mc-dim mb-2 text-xs font-semibold uppercase">{{ group.domain }}</h2>
      <div class="mc-panel divide-y" style="border-color: var(--border)">
        <div
          v-for="f in group.facts"
          :key="f.id"
          class="flex items-center gap-3 px-4 py-2.5 text-sm"
          :style="{ borderColor: 'var(--border)' }"
          data-test="fact-row"
        >
          <span class="font-medium">{{ ws.entityName(f.entity_id) }}</span>
          <span class="mc-dim">·</span>
          <span>{{ f.key }}</span>
          <span class="mc-dim">=</span>
          <span class="flex-1 truncate">{{ ws.displayValue(f.value) }}</span>
          <span :class="badgeClass(f.sensitivity)">{{ t(`sens.${f.sensitivity}`) }}</span>
          <span class="mc-dim text-xs">{{ t('profile.validFrom', { time: f.valid_from }) }}</span>
        </div>
      </div>
    </div>

    <p v-if="ws.facts.length === 0 && !ws.loading" class="mc-dim text-sm">
      {{ t('common.empty') }}
    </p>
  </section>
</template>
