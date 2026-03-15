<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { Trash2, FolderOpen, Hash, Tag } from 'lucide-vue-next'
import { useVideoStore } from '@/stores'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { openInExplorer, getDuplicateVideos } from '@/lib/tauri'
import { toast } from 'vue-sonner'
import type { Video } from '@/types'
import DeleteVideoDialog from './DeleteVideoDialog.vue'

interface Props {
  open?: boolean
}

interface Emits {
  (e: 'update:open', value: boolean): void
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
})

const emit = defineEmits<Emits>()

const videoStore = useVideoStore()

const isOpen = computed({
  get: () => props.open,
  set: (value) => emit('update:open', value),
})

// 重复类型
type DuplicateType = 'hash' | 'code' | 'both'

// 重复视频分组
interface DuplicateGroup {
  key: string           // 显示名称
  type: DuplicateType   // 重复类型
  isCrossDir: boolean   // 是否跨目录
  videos: Video[]
}

const duplicateGroups = ref<DuplicateGroup[]>([])
const loading = ref(false)

// 判断是否跨目录
const checkCrossDir = (videos: Video[]): boolean => {
  const dirs = new Set(videos.map(v => v.dirPath || v.videoPath.replace(/[\\/][^\\/]+$/, '')))
  return dirs.size > 1
}

// 查找重复视频
const findDuplicates = async () => {
  loading.value = true
  try {
    const videos = await getDuplicateVideos()
    
    const n = videos.length
    if (n === 0) {
      duplicateGroups.value = []
      loading.value = false
      return
    }

    // 并查集
    const parent = new Array(n).fill(0).map((_, i) => i)
    const find = (i: number): number => {
      if (parent[i] === i) return i
      parent[i] = find(parent[i])
      return parent[i]
    }
    const union = (i: number, j: number) => {
      const rootI = find(i)
      const rootJ = find(j)
      if (rootI !== rootJ) parent[rootI] = rootJ
    }

    // 记录每对合并的原因
    const hashMerged = new Set<string>() // 记录被 hash 合并的 root 对
    const codeMerged = new Set<string>() // 记录被番号合并的 root 对

    const hashToIdx = new Map<string, number>()
    const codeToIdx = new Map<string, number>()

    videos.forEach((video, i) => {
      // 基于 fastHash 去重
      if (video.fastHash) {
        if (hashToIdx.has(video.fastHash)) {
          const j = hashToIdx.get(video.fastHash)!
          hashMerged.add(`${Math.min(find(i), find(j))}-${Math.max(find(i), find(j))}`)
          union(i, j)
        } else {
          hashToIdx.set(video.fastHash, i)
        }
      }

      // 基于番号（localId）去重
      if (video.localId) {
        const code = video.localId.toLowerCase().trim()
        if (codeToIdx.has(code)) {
          const j = codeToIdx.get(code)!
          codeMerged.add(`${Math.min(find(i), find(j))}-${Math.max(find(i), find(j))}`)
          union(i, j)
        } else {
          codeToIdx.set(code, i)
        }
      }
    })

    // 分组
    const groups = new Map<number, number[]>()
    videos.forEach((_, i) => {
      const root = find(i)
      if (!groups.has(root)) groups.set(root, [])
      groups.get(root)!.push(i)
    })

    // 判断每组的重复类型
    const duplicates: DuplicateGroup[] = []
    groups.forEach((indices) => {
      if (indices.length <= 1) return

      const groupVideos = indices.map(i => videos[i])
      
      // 检查组内是否有 hash 重复和番号重复
      const hasHashDup = groupVideos.some((v, i) => 
        groupVideos.some((w, j) => i !== j && v.fastHash && w.fastHash && v.fastHash === w.fastHash)
      )
      const hasCodeDup = groupVideos.some((v, i) =>
        groupVideos.some((w, j) => i !== j && v.localId && w.localId && 
          v.localId.toLowerCase().trim() === w.localId.toLowerCase().trim())
      )

      const type: DuplicateType = hasHashDup && hasCodeDup ? 'both' : hasHashDup ? 'hash' : 'code'
      const isCrossDir = checkCrossDir(groupVideos)

      // 确定组名
      const validLocalId = groupVideos.find(v => v.localId)?.localId
      let key = ''

      if (validLocalId) {
        key = validLocalId
      } else if (hasHashDup) {
        key = `文件内容重复 (${groupVideos[0].fastHash?.substring(0, 8)})`
      } else {
        key = groupVideos[0].videoPath.split(/[\\/]/).pop() || '未知视频'
      }

      duplicates.push({
        key,
        type,
        isCrossDir,
        videos: groupVideos.sort((a, b) => (b.fileSize || 0) - (a.fileSize || 0)),
      })
    })

    // 排序：跨目录的排前面，然后按视频数量降序
    duplicates.sort((a, b) => {
      if (a.isCrossDir !== b.isCrossDir) return a.isCrossDir ? -1 : 1
      return b.videos.length - a.videos.length
    })

    duplicateGroups.value = duplicates
  } catch (e) {
    console.error('查找重复视频失败:', e)
    toast.error('查找重复视频失败')
  } finally {
    loading.value = false
  }
}

// 打开目录
const handleOpenDirectory = async (video: Video) => {
  try {
    await openInExplorer(video.videoPath)
  } catch (e) {
    console.error('打开目录失败:', e)
    toast.error('打开目录失败')
  }
}

// 删除视频
const videoToDelete = ref<Video | null>(null)
const showDeleteDialog = ref(false)

const handleDeleteVideo = (video: Video) => {
  videoToDelete.value = video
  showDeleteDialog.value = true
}

const handleDeleteSuccess = async () => {
  if (!videoToDelete.value) return
  const videoId = videoToDelete.value.id
  videoToDelete.value = null
  await findDuplicates()
  videoStore.removeVideo(videoId)
}

// 格式化文件大小
const formatFileSize = (bytes?: number) => {
  if (!bytes) return '-'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = bytes
  let unitIndex = 0
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024
    unitIndex++
  }
  return `${size.toFixed(2)} ${units[unitIndex]}`
}

// 获取视频标识
const getVideoLabel = (video: Video) => {
  return video.localId || video.title || video.videoPath.split(/[\\/]/).pop() || '未知'
}

// 获取目录名（用于跨目录显示）
const getDirName = (video: Video) => {
  const dir = video.dirPath || video.videoPath.replace(/[\\/][^\\/]+$/, '')
  return dir.split(/[\\/]/).pop() || dir
}

// 重复类型标签文本
const getTypeLabel = (type: DuplicateType) => {
  switch (type) {
    case 'hash': return '哈希重复'
    case 'code': return '番号重复'
    case 'both': return '哈希+番号重复'
  }
}

// 重复类型标签样式
const getTypeVariant = (type: DuplicateType) => {
  switch (type) {
    case 'hash': return 'destructive' as const
    case 'code': return 'default' as const
    case 'both': return 'destructive' as const
  }
}

// 统计信息
const stats = computed(() => ({
  totalGroups: duplicateGroups.value.length,
  totalVideos: duplicateGroups.value.reduce((sum, g) => sum + g.videos.length, 0),
  duplicateVideos: duplicateGroups.value.reduce((sum, g) => sum + (g.videos.length - 1), 0),
  crossDirGroups: duplicateGroups.value.filter(g => g.isCrossDir).length,
  hashGroups: duplicateGroups.value.filter(g => g.type === 'hash' || g.type === 'both').length,
  codeGroups: duplicateGroups.value.filter(g => g.type === 'code' || g.type === 'both').length,
}))

// 监听对话框打开，自动查找重复
watch(() => props.open, (newVal) => {
  if (newVal) {
    findDuplicates()
  }
})
</script>

<template>
  <Dialog v-model:open="isOpen">
    <DialogContent class="max-w-[90vw] w-[1000px] max-h-[85vh] flex flex-col sm:max-w-[1000px]">
      <DialogHeader>
        <DialogTitle>视频去重</DialogTitle>
        <DialogDescription>
          <template v-if="!loading && duplicateGroups.length > 0">
            检测到 {{ stats.totalGroups }} 组重复视频，共 {{ stats.duplicateVideos }} 个重复项
            <span v-if="stats.crossDirGroups > 0" class="ml-2">
              （{{ stats.crossDirGroups }} 组跨目录）
            </span>
          </template>
          <template v-else>
            跨目录检查哈希和番号重复
          </template>
        </DialogDescription>
      </DialogHeader>

      <!-- 统计标签 -->
      <div v-if="!loading && duplicateGroups.length > 0" class="flex items-center gap-2 flex-wrap">
        <Badge v-if="stats.hashGroups > 0" variant="destructive" class="text-xs">
          <Hash class="mr-1 size-3" />
          哈希重复 {{ stats.hashGroups }} 组
        </Badge>
        <Badge v-if="stats.codeGroups > 0" variant="default" class="text-xs">
          <Tag class="mr-1 size-3" />
          番号重复 {{ stats.codeGroups }} 组
        </Badge>
      </div>

      <!-- 空状态 -->
      <div v-if="!loading && duplicateGroups.length === 0" class="flex-1 flex items-center justify-center py-12">
        <div class="text-center text-muted-foreground">
          <p class="text-lg">未发现重复视频</p>
          <p class="text-sm mt-2">所有视频都是唯一的</p>
        </div>
      </div>

      <!-- 加载状态 -->
      <div v-if="loading" class="flex-1 flex items-center justify-center py-12">
        <div class="text-center text-muted-foreground">
          <p class="text-lg">正在跨目录查找重复视频...</p>
        </div>
      </div>

      <!-- 重复视频列表 -->
      <ScrollArea v-if="!loading && duplicateGroups.length > 0" class="flex-1 overflow-auto border rounded-md min-h-[400px]">
        <div class="space-y-6 p-4">
          <div v-for="group in duplicateGroups" :key="group.key" class="border rounded-lg p-4">
            <div class="mb-3 flex items-center gap-2 flex-wrap">
              <h3 class="font-semibold text-sm">{{ group.key }}</h3>
              <Badge :variant="getTypeVariant(group.type)" class="h-5 px-1.5 text-[10px]">
                {{ getTypeLabel(group.type) }}
              </Badge>
              <Badge v-if="group.isCrossDir" variant="outline" class="h-5 px-1.5 text-[10px]">
                跨目录
              </Badge>
              <span class="text-xs text-muted-foreground ml-auto">
                {{ group.videos.length }} 个文件
              </span>
            </div>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead class="w-[55%]">文件信息</TableHead>
                  <TableHead class="w-[45%] text-right pr-8">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-for="video in group.videos" :key="video.id">
                  <TableCell>
                    <div class="flex flex-col gap-1.5 max-w-[450px]">
                      <div class="font-medium text-sm truncate" :title="getVideoLabel(video)">
                        {{ getVideoLabel(video) }}
                      </div>
                      <div class="text-xs text-muted-foreground font-mono truncate opacity-80" :title="video.videoPath">
                        {{ video.videoPath }}
                      </div>
                      <div class="flex items-center gap-2 flex-wrap">
                        <Badge variant="secondary" class="h-5 px-1.5 text-[10px] font-normal">
                          {{ video.resolution || '未知分辨率' }}
                        </Badge>
                        <span class="text-xs text-muted-foreground">{{ formatFileSize(video.fileSize) }}</span>
                        <Badge v-if="group.isCrossDir" variant="outline" class="h-5 px-1.5 text-[10px] font-normal">
                          {{ getDirName(video) }}
                        </Badge>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell>
                    <div class="flex items-center justify-end gap-2 pr-4">
                      <Button
                        variant="outline"
                        size="sm"
                        class="h-8 px-3 shrink-0"
                        @click="handleOpenDirectory(video)"
                      >
                        <FolderOpen class="mr-1.5 size-4" />
                        打开目录
                      </Button>
                      <Button
                        variant="destructive"
                        size="sm"
                        class="h-8 px-3 shrink-0"
                        @click="handleDeleteVideo(video)"
                      >
                        <Trash2 class="mr-1.5 size-4" />
                        删除视频
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </div>
      </ScrollArea>
    </DialogContent>
  </Dialog>

  <!-- 删除确认对话框 -->
  <DeleteVideoDialog
    v-model:open="showDeleteDialog"
    :video="videoToDelete"
    @success="handleDeleteSuccess"
  />
</template>
