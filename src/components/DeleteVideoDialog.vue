<script setup lang="ts">
import { ref, computed, watch } from 'vue'
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
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
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
const deleteScrapeDataOnly = ref(false)

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

const isBatchDelete = computed(() => Boolean(props.videoIds && props.videoIds.length > 0))

const deleteDescription = computed(() => {
  if (deleteScrapeDataOnly.value) {
    return '将仅删除 NFO、封面图、预览图等刮削产物，并重置刮削状态，视频文件会保留。'
  }

  return '此操作将删除视频文件、NFO 文件、封面图和相关数据，无法恢复。'
})

const confirmText = computed(() => {
  if (isDeleting.value) {
    return deleteScrapeDataOnly.value ? '删除刮削数据中...' : '删除中...'
  }

  return deleteScrapeDataOnly.value ? '确认删除刮削数据' : '确认删除'
})

watch(
  () => props.open,
  (open) => {
    if (open) {
      deleteScrapeDataOnly.value = false
    }
  }
)

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
      await invoke('delete_videos', {
        ids: props.videoIds,
        deleteScrapeDataOnly: deleteScrapeDataOnly.value
      })

      if (deleteScrapeDataOnly.value) {
        await videoStore.fetchVideos()
      } else {
        props.videoIds.forEach(id => {
          videoStore.removeVideo(id)
        })
      }

      await videoStore.fetchDirectories()

      toast.success('删除成功', {
        description: deleteScrapeDataOnly.value
          ? `已删除 ${props.videoIds.length} 个视频的刮削数据`
          : `已删除 ${props.videoIds.length} 个视频`
      })
    }
    // 单个删除
    else if (props.video) {
      await invoke('delete_video_file', {
        id: props.video.id,
        deleteScrapeDataOnly: deleteScrapeDataOnly.value
      })

      if (deleteScrapeDataOnly.value) {
        await videoStore.fetchVideos()
      } else {
        videoStore.removeVideo(props.video.id)
      }

      await videoStore.fetchDirectories()

      toast.success('删除成功', {
        description: deleteScrapeDataOnly.value ? '视频刮削数据已删除' : '视频已删除'
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
        {{ deleteDescription }}
      </DialogDescription>
      <div class="mt-4 rounded-lg border border-border/70 bg-muted/30 px-3 py-3">
        <div class="flex items-start gap-3">
          <Checkbox id="delete-scrape-data-only" v-model="deleteScrapeDataOnly" :disabled="isDeleting" />
          <div class="space-y-1">
            <Label for="delete-scrape-data-only" class="cursor-pointer text-sm leading-5">
              不删除视频文件，仅删除刮削数据
            </Label>
            <p class="text-xs text-muted-foreground">
              {{ isBatchDelete ? '对所选视频生效，' : '' }}删除 .nfo、封面图、预览图等文件，保留视频本体。
            </p>
          </div>
        </div>
      </div>
      <div class="flex justify-end gap-3 mt-4">
        <Button variant="outline" :disabled="isDeleting" @click="handleCancel">
          取消
        </Button>
        <Button variant="destructive" :disabled="isDeleting" @click="handleConfirm">
          {{ confirmText }}
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
