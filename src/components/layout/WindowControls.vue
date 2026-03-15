<script setup lang="ts">
import { Minus, Square, X, Copy, Pin } from 'lucide-vue-next'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ref, onMounted } from 'vue'
import { useSettingsStore } from '@/stores/settings'

const appWindow = ref<ReturnType<typeof getCurrentWindow> | null>(null)
const isMaximized = ref(false)
const isAlwaysOnTop = ref(false)

const minimize = () => {
  appWindow.value?.minimize()
}

const toggleMaximize = async () => {
  if (!appWindow.value) return
  await appWindow.value.toggleMaximize()
  isMaximized.value = await appWindow.value.isMaximized()
}

const toggleFullscreen = async () => {
  if (!appWindow.value) return
  const isFullscreen = await appWindow.value.isFullscreen()
  await appWindow.value.setFullscreen(!isFullscreen)
}

const close = () => {
  appWindow.value?.close()
}

const toggleAlwaysOnTop = async () => {
  if (!appWindow.value) return
  try {
    const nextState = !isAlwaysOnTop.value
    await appWindow.value.setAlwaysOnTop(nextState)
    isAlwaysOnTop.value = nextState

    // 如果是视频播放器窗口，则保存设置
    if (appWindow.value.label.startsWith('video_player_')) {
      const settingsStore = useSettingsStore()
      settingsStore.updateSettings({
        videoPlayer: {
          alwaysOnTop: nextState
        }
      })
    }
  } catch (e) {
    console.error('Failed to toggle always on top', e)
  }
}

onMounted(async () => {
  try {
    appWindow.value = getCurrentWindow()
    if (appWindow.value) {
      isMaximized.value = await appWindow.value.isMaximized()

      // 如果是视频播放器窗口，初始化置顶状态
      if (appWindow.value.label.startsWith('video_player_')) {
        const settingsStore = useSettingsStore()
        const settings = await settingsStore.settings
        isAlwaysOnTop.value = settings.videoPlayer?.alwaysOnTop || false
      }
    }
  } catch {
    appWindow.value = null
  }
})

defineExpose({
  toggleAlwaysOnTop,
  toggleMaximize,
  toggleFullscreen
})
</script>

<template>
  <div class="flex items-center h-full" data-tauri-drag-region="false">
    <button @click="toggleAlwaysOnTop"
      class="inline-flex items-center justify-center h-full w-12 hover:bg-accent hover:text-accent-foreground transition-colors focus:outline-none"
      :class="isAlwaysOnTop ? 'text-primary' : ''" :title="isAlwaysOnTop ? '取消置顶' : '置顶'">
      <Pin class="size-4" :fill="isAlwaysOnTop ? 'currentColor' : 'none'" />
    </button>
    <button @click="minimize"
      class="inline-flex items-center justify-center h-full w-12 hover:bg-accent hover:text-accent-foreground transition-colors focus:outline-none"
      title="最小化">
      <Minus class="size-4" />
    </button>
    <button @click="toggleMaximize"
      class="inline-flex items-center justify-center h-full w-12 hover:bg-accent hover:text-accent-foreground transition-colors focus:outline-none"
      :title="isMaximized ? '还原' : '最大化'">
      <Copy v-if="isMaximized" class="size-4 rotate-180" />
      <Square v-else class="size-4" />
    </button>
    <button @click="close"
      class="inline-flex items-center justify-center h-full w-12 hover:bg-red-500 hover:text-white transition-colors focus:outline-none"
      title="关闭">
      <X class="size-4" />
    </button>
  </div>
</template>
