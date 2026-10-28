import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_facts') {
      return [
        {
          id: 1,
          entity_id: 1,
          key: 'height',
          value: '"175cm"',
          domain: 'health',
          sensitivity: 'normal',
          valid_from: '2026-01-01T00:00:00Z',
          valid_to: null,
          source_entry_id: null,
        },
      ]
    }
    if (cmd === 'list_entities') {
      return [{ id: 1, name: '我', kind: 'self', domain: null, status: 'active', created_at: '', updated_at: '' }]
    }
    return []
  }),
}))

import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { useLocaleStore } from '../stores/locale'
import ProfileView from './ProfileView.vue'

describe('ProfileView', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    useLocaleStore().setMode('zh')
  })

  it('按域分组展示事实，值去掉 JSON 引号', async () => {
    const wrapper = mount(ProfileView)
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain('health')
    expect(text).toContain('我')
    expect(text).toContain('height')
    expect(text).toContain('175cm')
    expect(text).not.toContain('"175cm"')
    expect(wrapper.findAll('[data-test="fact-row"]').length).toBe(1)
  })
})
