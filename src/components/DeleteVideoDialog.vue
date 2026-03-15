<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { useVideoStore } from '@/stores/video'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import type { Video } from '@/types'

interface Props {
  open: boolean
  video?: Video | null
  videoIds?: string[]
  videoTitle?: string
}

const props = defineProps<Props>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  'success': []
}>()

const videoStore = useVideoStore()
const isDeleting = ref(false)

const isOpen = computed({
  get: () => props.open,
  set: (value) => emit('update:open', value)
})

const displayTitle = computed(() => {
  if (props.videoTitle) return props.videoTitle
  if (props.video) return props.video.title || props.video.originalTitle
  if (props.videoIds && props.videoIds.length > 1) {
    return `${props.videoIds.length} 个视频`
  }
  return '该视频'
})

const handleCancel = () => {
  if (isDeleting.value) return
  isOpen.value = false
}

const handleConfirm = async () => {
  if (isDeleting.value) return

  isDeleting.value = true

  try {
    // 批量删除
    if (props.videoIds && props.videoIds.length > 0) {
      await invoke('delete_videos', { ids: props.videoIds })

      // 从 store 中移除
      props.videoIds.forEach(id => {
        videoStore.removeVideo(id)
      })

      await videoStore.fetchDirectories()

      toast.success('删除成功', {
        description: `已删除 ${props.videoIds.length} 个视频`
      })
    }
    // 单个删除
    else if (props.video) {
      await invoke('delete_video_file', { id: props.video.id })

      // 从 store 中移除
      videoStore.removeVideo(props.video.id)

      await videoStore.fetchDirectories()

      toast.success('删除成功', {
        description: '视频已删除'
      })
    }

    isOpen.value = false
    emit('success')
  } catch (e) {
    console.error('删除视频失败:', e)
    toast.error('删除失败', {
      description: (e as Error).message || '未知错误'
    })
  } finally {
    isDeleting.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="isOpen">
    <DialogContent class="sm:max-w-[425px]">
      <DialogTitle>确认删除</DialogTitle>
      <DialogDescription>
        确定要删除视频 "{{ displayTitle }}" 吗？
        <br /><br />
        此操作将删除视频文件、NFO文件、封面图和相关数据，<strong>无法恢复</strong>。
      </DialogDescription>
      <div class="flex justify-end gap-3 mt-4">
        <Button variant="outline" :disabled="isDeleting" @click="handleCancel">
          取消
        </Button>
        <Button variant="destructive" :disabled="isDeleting" @click="handleConfirm">
          {{ isDeleting ? '删除中...' : '确认删除' }}
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
