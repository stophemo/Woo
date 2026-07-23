import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

export type ThemeMode = 'light' | 'dark' | 'ocean' | 'forest' | 'rose'

export interface ThemeOption {
  id: ThemeMode
  label: string
  description: string
  isDark: boolean
  colors: [string, string, string]
}

export const THEME_OPTIONS: ThemeOption[] = [
  { id: 'light', label: '云白', description: '清爽的纸张质感', isDark: false, colors: ['#fbfcfc', '#eaf0f2', '#3d8aa8'] },
  { id: 'dark', label: '深夜', description: '低亮度深色界面', isDark: true, colors: ['#282a2d', '#202224', '#7ab0e0'] },
  { id: 'ocean', label: '海岸', description: '冷静的蓝绿色调', isDark: false, colors: ['#f8fcfc', '#e2eff1', '#248ca0'] },
  { id: 'forest', label: '松林', description: '柔和的自然绿色', isDark: false, colors: ['#fbfdfb', '#e5eee5', '#4c9664'] },
  { id: 'rose', label: '暮霞', description: '温暖的珊瑚粉色', isDark: false, colors: ['#fffafa', '#f3e8e6', '#c76b7b'] },
]

export function isDarkTheme(mode: ThemeMode): boolean {
  return mode === 'dark'
}

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
  return THEME_OPTIONS.some((option) => option.id === saved) ? saved : null
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
    theme.value = isDarkTheme(theme.value) ? 'light' : 'dark'
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
    toggleTheme,
    themeOptions: THEME_OPTIONS,
    isDarkTheme
  }
})
