<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useLocaleStore } from '../stores/locale'
import { useWorkspaceStore } from '../stores/workspace'
import type { ChatOp } from '../types'

const t = useLocaleStore().t
const ws = useWorkspaceStore()

const input = ref('')
const sending = ref(false)
const streamingText = ref('')
const streamError = ref('')
const messagesEl = ref<HTMLElement | null>(null)
const dismissed = ref(new Set<string>())

let unlisteners: UnlistenFn[] = []

const activeConv = computed(
  () => ws.conversations.find((c) => c.id === ws.activeConversationId) ?? null,
)
const cloudReady = computed(() => !!ws.settings['cloud_base_url'] && !!ws.settings['cloud_model'])

function scrollToBottom() {
  void nextTick(() => {
    if (messagesEl.value) messagesEl.value.scrollTop = messagesEl.value.scrollHeight
  })
}

async function send() {
  const content = input.value.trim()
  if (!content || sending.value || !ws.activeConversationId) return
  if (activeConv.value?.backend === 'cloud' && !cloudReady.value) {
    streamError.value = t('chat.notConfigured')
    return
  }
  input.value = ''
  streamError.value = ''
  streamingText.value = ''
  sending.value = true
  scrollToBottom()
  try {
    await ws.sendMessage(content)
  } catch (e) {
    streamError.value = t('chat.streamError', { msg: String(e) })
    sending.value = false
  }
}

function newConversation(backend: 'cloud' | 'local') {
  void ws.newConversation(backend).then(scrollToBottom)
}

function opLabel(op: ChatOp): string {
  return op.type === 'upsert_fact' ? t('chat.opFact') : t('chat.opEntry')
}
function opSummary(op: ChatOp): string {
  if (op.type === 'upsert_fact') {
    const v = typeof op.value === 'string' ? op.value : JSON.stringify(op.value)
    return `${op.entity} · ${op.key} = ${v}`
  }
  return `${op.title}（${op.body.slice(0, 60)}）`
}
function opKey(messageId: number, index: number): string {
  return `${messageId}:${index}`
}

async function adopt(messageId: number, index: number, op: ChatOp) {
  try {
    await ws.applyOp(messageId, index, op)
    const msg = ws.messages.find((m) => m.id === messageId)
    if (msg && !msg.adopted.includes(index)) msg.adopted.push(index)
  } catch (e) {
    streamError.value = t('chat.streamError', { msg: String(e) })
  }
}
function dismiss(messageId: number, index: number) {
  dismissed.value.add(opKey(messageId, index))
}

onMounted(async () => {
  if (ws.conversations.length === 0) await ws.loadConversations()
  if (Object.keys(ws.settings).length === 0) await ws.loadSettings()
  if (ws.activeConversationId === null && ws.conversations.length > 0) {
    await ws.openConversation(ws.conversations[0].id)
  }
  scrollToBottom()

  unlisteners.push(
    await listen<{ conversation_id: number; delta: string }>('memoir://chat-chunk', (e) => {
      if (e.payload.conversation_id !== ws.activeConversationId) return
      streamingText.value += e.payload.delta
      scrollToBottom()
    }),
  )
  unlisteners.push(
    await listen<{ conversation_id: number }>('memoir://chat-done', async (e) => {
      if (e.payload.conversation_id !== ws.activeConversationId) return
      streamingText.value = ''
      sending.value = false
      await ws.loadConversations()
      await ws.openConversation(e.payload.conversation_id)
      scrollToBottom()
    }),
  )
  unlisteners.push(
    await listen<{ conversation_id: number; error: string }>('memoir://chat-error', (e) => {
      if (e.payload.conversation_id !== ws.activeConversationId) return
      streamError.value = t('chat.streamError', { msg: e.payload.error })
      streamingText.value = ''
      sending.value = false
    }),
  )
})
onBeforeUnmount(() => unlisteners.forEach((u) => u()))

watch(
  () => ws.activeConversationId,
  () => {
    streamingText.value = ''
    streamError.value = ''
    dismissed.value = new Set()
    scrollToBottom()
  },
)
</script>

<template>
  <div class="mx-auto flex h-full max-w-4xl gap-4" data-test="chat-view">
    <aside class="flex w-52 shrink-0 flex-col gap-2">
      <div class="flex gap-2">
        <button class="mc-btn mc-btn-primary flex-1 text-xs" @click="newConversation('cloud')">
          + {{ t('chat.cloud') }}
        </button>
        <button class="mc-btn flex-1 text-xs" @click="newConversation('local')">
          + {{ t('chat.local') }}
        </button>
      </div>
      <div class="mc-panel flex-1 overflow-y-auto divide-y" style="border-color: var(--border)">
        <button
          v-for="c in ws.conversations"
          :key="c.id"
          class="w-full px-3 py-2 text-left text-xs"
          :style="c.id === ws.activeConversationId ? 'background: var(--bg-hover)' : ''"
          @click="ws.openConversation(c.id)"
        >
          <div class="truncate">{{ c.title }}</div>
          <div class="mc-dim mt-0.5 flex items-center gap-1">
            <span class="mc-badge">{{ t(c.backend === 'cloud' ? 'chat.cloud' : 'chat.local') }}</span>
            <button
              class="ml-auto"
              style="color: #ef4444"
              @click.stop="ws.deleteConversation(c.id)"
            >
              ✕
            </button>
          </div>
        </button>
        <p v-if="ws.conversations.length === 0" class="mc-dim p-3 text-xs">{{ t('chat.empty') }}</p>
      </div>
    </aside>

    <section class="flex min-w-0 flex-1 flex-col">
      <div ref="messagesEl" class="mc-panel flex-1 overflow-y-auto p-4">
        <template v-for="m in ws.messages" :key="m.id">
          <div v-if="m.role === 'user'" class="mb-3 flex justify-end">
            <div
              class="max-w-[75%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap"
              style="background: var(--accent); color: #fff"
            >
              {{ m.content }}
            </div>
          </div>
          <div v-else-if="m.role === 'assistant'" class="mb-3 flex flex-col gap-2">
            <template v-for="(seg, i) in m.segments" :key="i">
              <div
                v-if="seg.kind === 'text'"
                class="max-w-[85%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap"
                style="background: var(--bg-hover)"
              >
                {{ seg.content }}
              </div>
              <div
                v-else-if="!dismissed.has(opKey(m.id, seg.index))"
                class="max-w-[85%] rounded-lg border p-3 text-sm"
                style="border-color: var(--accent)"
                data-test="op-card"
              >
                <div class="mb-1 flex items-center gap-2">
                  <span class="mc-badge" style="color: var(--accent)">{{ opLabel(seg.op) }}</span>
                  <span v-if="m.adopted.includes(seg.index)" class="mc-dim text-xs">
                    ✓ {{ t('chat.adopted') }}
                  </span>
                </div>
                <div class="mb-2 text-xs">{{ opSummary(seg.op) }}</div>
                <div v-if="!m.adopted.includes(seg.index)" class="flex gap-2">
                  <button
                    class="mc-btn mc-btn-primary text-xs"
                    @click="adopt(m.id, seg.index, seg.op)"
                  >
                    {{ t('chat.adopt') }}
                  </button>
                  <button class="mc-btn text-xs" @click="dismiss(m.id, seg.index)">
                    {{ t('chat.dismiss') }}
                  </button>
                </div>
              </div>
            </template>
          </div>
        </template>

        <div
          v-if="streamingText"
          class="mb-3 max-w-[85%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap"
          style="background: var(--bg-hover)"
        >
          {{ streamingText }}
        </div>
        <p v-if="ws.messages.length === 0 && !streamingText && activeConv" class="mc-dim text-sm">
          {{ t('chat.purgeHint') }}
        </p>
      </div>

      <p v-if="streamError" class="mt-2 text-xs" style="color: #ef4444">{{ streamError }}</p>

      <div class="mt-3 flex items-end gap-2">
        <textarea
          v-model="input"
          class="mc-textarea flex-1"
          rows="2"
          :placeholder="t('chat.placeholder')"
          data-test="chat-input"
          @keydown.enter.exact.prevent="send"
        ></textarea>
        <button class="mc-btn mc-btn-primary text-sm" :disabled="sending" @click="send">
          {{ t('chat.send') }}
        </button>
      </div>
    </section>
  </div>
</template>
