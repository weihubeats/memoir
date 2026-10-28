import { describe, expect, it } from 'vitest'
import { zh } from '../i18n/zh'
import { en } from '../i18n/en'

describe('i18n 字典', () => {
  it('zh 与 en 键集 1:1', () => {
    const zhKeys = Object.keys(zh).sort()
    const enKeys = Object.keys(en).sort()
    expect(enKeys).toEqual(zhKeys)
  })

  it('占位符在两种语言中一致', () => {
    for (const key of Object.keys(zh) as (keyof typeof zh)[]) {
      const zhVars = [...zh[key].matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort()
      const enVars = [...en[key].matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort()
      expect(enVars, `key: ${key}`).toEqual(zhVars)
    }
  })
})
