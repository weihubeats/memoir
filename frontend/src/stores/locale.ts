import { defineStore } from 'pinia'
import { zh, type I18nKey } from '../i18n/zh'
import { en } from '../i18n/en'

export type LocaleMode = 'zh' | 'en'

export const useLocaleStore = defineStore('locale', {
  state: () => ({
    mode: (localStorage.getItem('mc-locale') as LocaleMode) || 'zh',
  }),
  getters: {
    t(state) {
      return (key: I18nKey, vars?: Record<string, string | number>): string => {
        const dict = state.mode === 'en' ? en : zh
        let text = dict[key] ?? zh[key] ?? key
        if (vars) {
          for (const [k, v] of Object.entries(vars)) {
            text = text.replaceAll(`{${k}}`, String(v))
          }
        }
        return text
      }
    },
  },
  actions: {
    setMode(mode: LocaleMode) {
      this.mode = mode
      localStorage.setItem('mc-locale', mode)
    },
  },
})
