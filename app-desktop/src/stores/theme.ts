import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

export type ThemeMode = 'light' | 'dark'

const STORAGE_KEY = 'non-ego-notes-theme'

export const useThemeStore = defineStore('theme', () => {
  const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null
  const theme = ref<ThemeMode>(saved === 'dark' ? 'dark' : 'light')

  function applyTheme(mode: ThemeMode) {
    document.documentElement.setAttribute('data-theme', mode)
  }

  function toggleTheme() {
    theme.value = theme.value === 'light' ? 'dark' : 'light'
  }

  // 持久化 + 同步 DOM
  watch(theme, (newTheme) => {
    applyTheme(newTheme)
    localStorage.setItem(STORAGE_KEY, newTheme)
  }, { immediate: true })

  return {
    theme,
    toggleTheme
  }
})
