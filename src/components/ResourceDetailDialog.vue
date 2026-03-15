<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { ResourceItem } from '@/types/resourceSearch'
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Download,
  Image as ImageIcon,
  X,
  Star,
  Loader2,
} from 'lucide-vue-next'
import { openImagePreview, openLongScreenshot, isFancyboxOpen } from '@/composables/useImagePreview'
import { toImageSrc } from '@/utils/image'
import type { PreviewImage } from '@/composables/useImagePreview'
import { toast } from 'vue-sonner'

interface Props {
  open: boolean
  resource: ResourceItem | null
}

const props = defineProps<Props>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  'find-links': [code: string]
}>()

const isOpen = computed({
  get: () => props.open,
  set: (value) => emit('update:open', value),
})

// Fancybox 打开时阻止外部点击关闭详情页
const onInteractOutside = (e: Event) => {
  if (isFancyboxOpen()) e.preventDefault()
}

// ============ 图片加载状态 ============
const coverLoading = ref(true)
const thumbLoadingSet = ref(new Set<number>())
const longScreenshotLoading = ref(false)

/** 所有可查看的图片（封面 + 截图） */
const allImages = computed<PreviewImage[]>(() => {
  const images: PreviewImage[] = []
  if (props.resource?.coverUrl) {
    const src = toImageSrc(props.resource.coverUrl) ?? props.resource.coverUrl
    images.push({ src, title: '封面' })
  }
  if (props.resource?.screenshots) {
    props.resource.screenshots.forEach((url, idx) => {
      const src = toImageSrc(url) ?? url
      images.push({ src, title: `截图 ${idx + 1}` })
    })
  }
  return images
})

/** 截图列表 */
const screenshots = computed(() => props.resource?.screenshots ?? [])

// 关闭对话框时重置加载状态
watch(() => props.open, (val) => {
  if (!val) {
    coverLoading.value = true
    thumbLoadingSet.value = new Set()
  } else {
    coverLoading.value = true
    thumbLoadingSet.value = new Set(
      Array.from({ length: screenshots.value.length }, (_, i) => i)
    )
  }
})

/** 缺失字段显示占位符 */
function displayValue(value: string | undefined): string {
  return value || '-'
}

/** 打开图片查看器（使用 Fancybox） */
function openImageViewer(index: number) {
  if (allImages.value.length === 0) return
  openImagePreview(allImages.value, index)
}

/** 查找视频下载链接 */
function handleFindDownload() {
  if (!props.resource?.code) {
    toast.error('当前资源没有番号信息')
    return
  }
  emit('find-links', props.resource.code)
  isOpen.value = false
}

/** 查看视频长截图（使用 Fancybox） */
function handleViewScreenshot() {
  if (!props.resource?.code) return
  const code = props.resource.code.toUpperCase()
  const url = `https://memojav.com/image/screenshot/${code}.jpg`
  longScreenshotLoading.value = true
  openLongScreenshot(
    url,
    `长截图 · ${code}`,
    () => {
      // 图片加载失败（404 等），toast 提示用户
      toast.error('该番号暂无长截图，图片资源不存在或无法访问')
    },
  )
  // openLongScreenshot 内部通过 Image 探测，无论成功失败都会回调，这里统一重置 loading
  // 由于 Image 加载是异步的，用同一个 url 会命中浏览器缓存，几乎同步返回
  const probe = new Image()
  probe.onload = probe.onerror = () => { longScreenshotLoading.value = false }
  probe.src = url
}
</script>

<template>
  <Dialog v-model:open="isOpen">
    <DialogContent
      class="sm:max-w-[1000px] h-[85vh] flex flex-col p-0 gap-0 overflow-hidden"
      aria-describedby="resource-dialog-desc"
      @interact-outside="onInteractOutside"
    >
      <DialogTitle class="sr-only">视频详情</DialogTitle>
      <DialogDescription id="resource-dialog-desc" class="sr-only">
        查看资源搜索结果的详细信息
      </DialogDescription>

      <!-- 上下布局 -->
      <div class="flex flex-col flex-1 overflow-hidden">
        <!-- 上部：封面 + 详情信息（横向排列） -->
        <ScrollArea class="flex-1 min-h-0">
          <div class="p-6 pt-10 space-y-4">
            <div class="flex gap-6">
              <!-- 封面 -->
              <div class="shrink-0">
                <div
                  class="w-[260px] min-h-[180px] rounded-lg overflow-hidden shadow-md relative bg-black/5 flex items-center justify-center transition-all"
                  :class="resource?.coverUrl ? 'cursor-pointer hover:ring-2 hover:ring-primary' : ''"
                  @click="resource?.coverUrl && openImageViewer(0)"
                >
                  <img
                    v-if="resource?.coverUrl"
                    :src="toImageSrc(resource.coverUrl) ?? ''"
                    class="w-full h-auto object-contain max-h-[220px]"
                    :class="coverLoading ? 'opacity-0 absolute' : 'opacity-100'"
                    referrerPolicy="no-referrer"
                    @load="coverLoading = false"
                    @error="coverLoading = false"
                  />
                  <!-- 封面加载中 -->
                  <div
                    v-if="resource?.coverUrl && coverLoading"
                    class="flex flex-col items-center justify-center text-muted-foreground gap-2"
                  >
                    <Loader2 class="size-6 animate-spin opacity-40" />
                    <span class="text-xs opacity-40">加载中</span>
                  </div>
                  <!-- 暂无封面 -->
                  <div
                    v-else-if="!resource?.coverUrl"
                    class="flex flex-col items-center justify-center text-muted-foreground p-8 gap-3"
                  >
                    <ImageIcon class="size-12 opacity-20" />
                    <span class="text-xs">暂无封面</span>
                  </div>
                </div>
              </div>

              <!-- 详情信息 -->
              <div class="flex-1 min-w-0 space-y-3">
                <!-- 标题 -->
                <div class="text-lg font-bold leading-snug">
                  {{ displayValue(resource?.title) }}
                </div>

                <!-- 第一行：番号、发行日期、时长 -->
                <div class="grid grid-cols-3 gap-x-4 gap-y-3">
                  <div class="space-y-1">
                    <Label class="text-[10px] text-muted-foreground uppercase tracking-wider">番号 (ID)</Label>
                    <div class="font-mono text-sm">{{ displayValue(resource?.code) }}</div>
                  </div>
                  <div class="space-y-1">
                    <Label class="text-[10px] text-muted-foreground uppercase tracking-wider">发行日期</Label>
                    <div class="text-sm">{{ displayValue(resource?.premiered) }}</div>
                  </div>
                  <div class="space-y-1">
                    <Label class="text-[10px] text-muted-foreground uppercase tracking-wider">时长</Label>
                    <div class="text-sm">{{ displayValue(resource?.duration) }}</div>
                  </div>
                </div>

                <!-- 第二行：制作商、导演、演员 -->
                <div class="grid grid-cols-3 gap-x-4 gap-y-3">
                  <div class="space-y-1">
                    <Label class="text-[10px] text-muted-foreground uppercase tracking-wider">制作商</Label>
                    <div class="text-sm">{{ displayValue(resource?.studio) }}</div>
                  </div>
                  <div class="space-y-1">
                    <Label class="text-[10px] text-muted-foreground uppercase tracking-wider">导演</Label>
                    <div class="text-sm">{{ displayValue(resource?.director) }}</div>
                  </div>
                  <div class="space-y-1">
                    <Label class="text-[10px] text-muted-foreground uppercase tracking-wider">演员</Label>
                    <div class="text-sm">{{ displayValue(resource?.actors) }}</div>
                  </div>
                </div>

                <!-- 第三行：标签 -->
                <div class="space-y-1">
                  <Label class="text-[10px] text-muted-foreground uppercase tracking-wider">标签 / 分类</Label>
                  <div class="text-sm">{{ displayValue(resource?.tags) }}</div>
                </div>

                <!-- 第四行：评分 -->
                <div class="space-y-1">
                  <Label class="text-[10px] text-muted-foreground uppercase tracking-wider">评分</Label>
                  <div class="flex items-center gap-1">
                    <template v-if="resource?.rating">
                      <Star
                        v-for="i in 10"
                        :key="i"
                        class="size-4"
                        :class="(resource.rating ?? 0) >= i ? 'text-yellow-500 fill-yellow-500' : 'text-muted-foreground/30'"
                      />
                      <span class="ml-1 text-sm text-muted-foreground">{{ resource.rating }}</span>
                    </template>
                    <span v-else class="text-sm">-</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </ScrollArea>

        <!-- 预览图：横向滚动，宽度撑满 -->
        <div class="shrink-0 border-t px-6 py-3">
          <div class="flex items-center gap-2 mb-2">
            <ImageIcon class="size-4 text-muted-foreground" />
            <span class="text-xs font-medium text-muted-foreground">预览图</span>
          </div>
          <div
            v-if="screenshots.length > 0"
            class="flex gap-2 overflow-x-auto pb-1"
            @wheel.prevent="(e: WheelEvent) => (e.currentTarget as HTMLElement).scrollLeft += e.deltaY"
          >
            <div
              v-for="(src, idx) in screenshots"
              :key="idx"
              class="shrink-0 h-[140px] rounded-md overflow-hidden border shadow-sm bg-black/5 cursor-pointer hover:ring-2 hover:ring-primary transition-all relative"
              @click="openImageViewer((resource?.coverUrl ? 1 : 0) + idx)"
            >
              <img
                :src="src"
                class="h-full w-auto object-cover transition-opacity duration-200"
                :class="thumbLoadingSet.has(idx) ? 'opacity-0' : 'opacity-100'"
                loading="lazy"
                referrerPolicy="no-referrer"
                @load="thumbLoadingSet.delete(idx); thumbLoadingSet = new Set(thumbLoadingSet)"
                @error="thumbLoadingSet.delete(idx); thumbLoadingSet = new Set(thumbLoadingSet)"
              />
              <!-- 缩略图加载中 -->
              <div
                v-if="thumbLoadingSet.has(idx)"
                class="absolute inset-0 flex items-center justify-center"
              >
                <Loader2 class="size-4 animate-spin text-muted-foreground opacity-40" />
              </div>
            </div>
          </div>
          <div
            v-else
            class="flex items-center justify-center h-[140px] text-muted-foreground border rounded-md bg-black/5"
          >
            <span class="text-xs">暂无截图</span>
          </div>
        </div>

        <!-- 底部按钮 -->
        <div class="shrink-0 p-4 border-t bg-muted/20 flex items-center gap-3">
          <Button variant="default" size="sm" @click="handleFindDownload">
            <Download class="mr-2 size-4" />
            查找视频下载链接
          </Button>
          <Button variant="outline" size="sm" :disabled="longScreenshotLoading" @click="handleViewScreenshot">
            <Loader2 v-if="longScreenshotLoading" class="mr-2 size-4 animate-spin" />
            <ImageIcon v-else class="mr-2 size-4" />
            {{ longScreenshotLoading ? '加载中...' : '查看视频长截图' }}
          </Button>
          <div class="flex-1" />
          <Button variant="ghost" size="sm" class="border" @click="isOpen = false">
            <X class="mr-2 size-4" />
            关闭
          </Button>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
