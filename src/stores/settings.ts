// 设置状态管理
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { AppSettings, ThemeMode } from '@/types'
import { defaultSettings } from '@/types'
import { getSettings, saveSettings } from '@/lib/tauri'

export const useSettingsStore = defineStore('settings', () => {
    // ============ State ============
    const settings = ref<AppSettings>({ ...defaultSettings })
    const loading = ref(false)
    const saving = ref(false)
    const error = ref<string | null>(null)

    // ============ Theme Management ============
    function applyTheme(mode: ThemeMode) {
        const root = document.documentElement

        if (mode === 'system') {
            const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
            root.classList.toggle('dark', prefersDark)
        } else {
            root.classList.toggle('dark', mode === 'dark')
        }
    }

    // 监听系统主题变化
    if (typeof window !== 'undefined') {
        const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
        mediaQuery.addEventListener('change', () => {
            if (settings.value.theme.mode === 'system') {
                applyTheme('system')
            }
        })
    }

    // ============ Actions ============
    async function loadSettings() {
        loading.value = true
        error.value = null

        try {
            const loaded = await getSettings()
            // 深度合并设置，确保所有嵌套字段都有默认值
            settings.value = {
                theme: {
                    ...defaultSettings.theme,
                    ...loaded.theme,
                    proxy: { ...defaultSettings.theme.proxy, ...loaded.theme?.proxy }
                },
                general: { ...defaultSettings.general, ...loaded.general },
                download: { ...defaultSettings.download, ...loaded.download },
                scrape: { ...defaultSettings.scrape, ...loaded.scrape },
                ai: { ...defaultSettings.ai, ...loaded.ai },
                videoPlayer: { ...defaultSettings.videoPlayer, ...loaded.videoPlayer },
                mainWindow: { ...defaultSettings.mainWindow, ...loaded.mainWindow },
            }
            applyTheme(settings.value.theme.mode)
        } catch (e) {
            // 如果加载失败，使用默认设置
            console.warn('Failed to load settings, using defaults:', e)
            settings.value = { ...defaultSettings }
            applyTheme(settings.value.theme.mode)
        } finally {
            loading.value = false
        }
    }

    async function updateSettings(newSettings: Partial<AppSettings>) {
        saving.value = true
        error.value = null

        try {
            // 深度合并设置，确保嵌套对象正确合并
            settings.value = {
                ...settings.value,
                ...newSettings,
                theme: newSettings.theme ? { ...settings.value.theme, ...newSettings.theme } : settings.value.theme,
                general: newSettings.general ? { ...settings.value.general, ...newSettings.general } : settings.value.general,
                download: newSettings.download ? { ...settings.value.download, ...newSettings.download } : settings.value.download,
                scrape: newSettings.scrape ? { ...settings.value.scrape, ...newSettings.scrape } : settings.value.scrape,
                ai: newSettings.ai ? { ...settings.value.ai, ...newSettings.ai } : settings.value.ai,
                videoPlayer: newSettings.videoPlayer ? { ...settings.value.videoPlayer, ...newSettings.videoPlayer } : settings.value.videoPlayer,
                mainWindow: newSettings.mainWindow ? { ...settings.value.mainWindow, ...newSettings.mainWindow } : settings.value.mainWindow,
            }

            await saveSettings(settings.value)

            // 如果主题变化，立即应用
            if (newSettings.theme?.mode) {
                applyTheme(newSettings.theme.mode)
            }
        } catch (e) {
            error.value = (e as Error).message
            console.error('Failed to save settings:', e)
        } finally {
            saving.value = false
        }
    }

    function setThemeMode(mode: ThemeMode) {
        settings.value.theme.mode = mode
        applyTheme(mode)
        updateSettings({ theme: settings.value.theme })
    }

    function resetSettings() {
        settings.value = { ...defaultSettings }
        applyTheme(settings.value.theme.mode)
        updateSettings(settings.value)
    }

    return {
        // State
        settings,
        loading,
        saving,
        error,
        // Actions
        loadSettings,
        updateSettings,
        setThemeMode,
        resetSettings,
        applyTheme,
    }
})
