import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

export type ThemeMode = 'light' | 'dark'

function getSystemTheme(): ThemeMode {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

const STORAGE_KEY = 'non-ego-notes-theme'

/** 用户是否手动切换过主题 */
function hasUserPreference(): boolean {
  return localStorage.getItem(STORAGE_KEY) !== null
}

function loadSavedTheme(): ThemeMode | null {
  const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null
  return saved === 'dark' || saved === 'light' ? saved : null
}

export const useThemeStore = defineStore('theme', () => {
  const saved = loadSavedTheme()
  // 无保存偏好则跟随系统
  const theme = ref<ThemeMode>(saved ?? getSystemTheme())

  let mediaQuery: MediaQueryList | null = null
  let mediaListener: ((this: MediaQueryList, ev: MediaQueryListEvent) => void) | null = null

  function applyTheme(mode: ThemeMode) {
    document.documentElement.setAttribute('data-theme', mode)
  }

  function toggleTheme() {
    theme.value = theme.value === 'light' ? 'dark' : 'light'
  }

  // 监听系统主题变化（仅当用户未手动保存偏好时自动跟随）
  function setupSystemListener() {
    mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
    mediaListener = (e: MediaQueryListEvent) => {
      // 如果用户从未手动切换，跟随系统变化
      if (!hasUserPreference()) {
        theme.value = e.matches ? 'dark' : 'light'
      }
    }
    mediaQuery.addEventListener('change', mediaListener)
  }

  setupSystemListener()

  // 持久化 + 同步 DOM
  watch(theme, (newTheme) => {
    applyTheme(newTheme)
    // 仅保存本次 value（用户手动切换结果），系统自动变化不保存
    if (hasUserPreference() || newTheme !== getSystemTheme()) {
      localStorage.setItem(STORAGE_KEY, newTheme)
    }
  }, { immediate: true })

  return {
    theme,
    toggleTheme
  }
})
