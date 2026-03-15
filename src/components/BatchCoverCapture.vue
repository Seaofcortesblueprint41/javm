<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Play, Square, Camera, Trash2, FolderOpen, RotateCcw, ChevronDown } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { toast } from 'vue-sonner'
import { useResourceScrapeStore } from '@/stores/resourceScrape'
import { openInExplorer, openWithPlayer } from '@/lib/tauri'

// 截图封面任务
interface CoverTask {
  id: string
  videoId: string
  videoPath: string
  status: 'waiting' | 'running' | 'completed' | 'failed'
  coverPath?: string
  error?: string
}

// 进度事件
interface CoverCaptureProgress {
  taskId: string
  videoId: string
  status: string
  coverPath?: string
  error?: string
  completed: number
  total: number
}

const store = useResourceScrapeStore()
const tasks = ref<CoverTask[]>([])
const isRunning = ref(false)

// 事件监听
const unlisteners: UnlistenFn[] = []

// 从数据库加载任务
async function fetchTasks() {
  try {
    const result = await invoke<Array<Record<string, any>>>('rs_get_cover_capture_tasks')
    tasks.value = result.map(t => ({
      id: t.id,
      videoId: t.videoId,
      videoPath: t.videoPath,
      status: t.status as CoverTask['status'],
      coverPath: t.coverPath || undefined,
      error: t.error || undefined,
    }))
  } catch (e) {
    console.error('加载截图封面任务失败:', e)
  }
}

// 监听截图进度
listen<CoverCaptureProgress>('cover-capture-progress', (event) => {
  const { videoId, status, coverPath, error } = event.payload
  const task = tasks.value.find(t => t.videoId === videoId)
  if (task) {
    task.status = status as CoverTask['status']
    if (coverPath) task.coverPath = coverPath
    if (error) task.error = error
  }
}).then(fn => unlisteners.push(fn))

// 监听完成事件
listen('cover-capture-done', () => {
  isRunning.value = false
  fetchTasks() // 从数据库刷新最终状态
  toast.success('批量截图封面完成')
}).then(fn => unlisteners.push(fn))

onMounted(() => {
  fetchTasks()
})

onUnmounted(() => {
  unlisteners.forEach(fn => fn())
})

// 统计
const stats = computed(() => {
  const total = tasks.value.length
  const waiting = tasks.value.filter(t => t.status === 'waiting').length
  const running = tasks.value.filter(t => t.status === 'running').length
  const completed = tasks.value.filter(t => t.status === 'completed').length
  const failed = tasks.value.filter(t => t.status === 'failed').length
  return { total, waiting, running, completed, failed }
})

const progressPercent = computed(() => {
  if (stats.value.total === 0) return 0
  return (stats.value.completed / stats.value.total) * 100
})

// 从目录管理右键菜单添加任务（持久化到数据库）
async function addFromPath(path: string) {
  try {
    const count = await store.createCoverCaptureTasks(path)
    if (count > 0) {
      toast.success(`已添加 ${count} 个无封面视频到批量截图封面`)
      await fetchTasks()
    } else {
      toast.info('该目录下所有视频都已有封面或已在任务列表中')
    }
  } catch (e) {
    toast.error('添加失败', { description: (e as Error).message })
  }
}

// 开始批量截图
async function startCapture() {
  const pendingCount = stats.value.waiting + stats.value.failed
  if (pendingCount === 0) {
    toast.info('没有等待处理的任务')
    return
  }

  isRunning.value = true

  try {
    await invoke('rs_batch_capture_covers', { concurrency: 4 })
  } catch (e) {
    isRunning.value = false
    toast.error('启动批量截图失败', { description: (e as Error).message })
  }
}

// 停止截图
async function stopCapture() {
  try {
    await invoke('rs_stop_cover_capture')
    isRunning.value = false
    await fetchTasks()
    toast.info('已停止批量截图')
  } catch (e) {
    toast.error('停止失败', { description: (e as Error).message })
  }
}

// 清除已完成
async function clearCompleted() {
  try {
    const count = await invoke<number>('rs_delete_completed_cover_tasks')
    await fetchTasks()

    if (count > 0) {
      toast.success(`已删除 ${count} 个完成任务`)
    } else {
      toast.info('没有完成任务可删除')
    }
  } catch (e) {
    toast.error('删除失败', { description: (e as Error).message })
  }
}

// 删除失败任务
async function clearFailed() {
  if (stats.value.failed === 0) return
  try {
    const count = await invoke<number>('rs_delete_failed_cover_tasks')
    await fetchTasks()

    if (count > 0) {
      toast.success(`已删除 ${count} 个失败任务`)
    } else {
      toast.info('没有失败任务可删除')
    }
  } catch (e) {
    toast.error('删除失败', { description: (e as Error).message })
  }
}

// 删除全部任务
async function clearAll() {
  if (isRunning.value) {
    toast.warning('请先停止运行中的任务')
    return
  }
  try {
    const count = await invoke<number>('rs_delete_all_cover_tasks')
    await fetchTasks()

    if (count > 0) {
      toast.success(`已删除 ${count} 个任务`)
    } else {
      toast.info('没有任务可删除')
    }
  } catch (e) {
    toast.error('删除失败', { description: (e as Error).message })
  }
}

// 右键菜单：播放视频
function handlePlay(task: CoverTask) {
  openWithPlayer(task.videoPath)
}

// 右键菜单：打开目录
function handleOpenFolder(task: CoverTask) {
  openInExplorer(task.videoPath)
}

// 右键菜单：重试
async function handleRetry(task: CoverTask) {
  try {
    await invoke('rs_retry_cover_task', { videoId: task.videoId })
    task.status = 'waiting'
    task.error = undefined
  } catch (e) {
    toast.error('重试失败', { description: (e as Error).message })
  }
}

// 右键菜单：删除
async function handleDelete(task: CoverTask) {
  try {
    await invoke('rs_delete_cover_task', { videoId: task.videoId })
    tasks.value = tasks.value.filter(t => t.videoId !== task.videoId)
  } catch (e) {
    toast.error('删除失败', { description: (e as Error).message })
  }
}

// 获取文件名
function getFileName(path: string) {
  return path.split(/[/\\]/).pop() || path
}

// 状态颜色
function getStatusVariant(status: string): 'default' | 'secondary' | 'destructive' | 'outline' {
  switch (status) {
    case 'running': return 'default'
    case 'completed': return 'secondary'
    case 'failed': return 'destructive'
    default: return 'outline'
  }
}

// 状态文本
function getStatusText(status: string) {
  switch (status) {
    case 'waiting': return '等待中'
    case 'running': return '截图中'
    case 'completed': return '已完成'
    case 'failed': return '失败'
    default: return '未知'
  }
}

// 暴露方法给父组件
defineExpose({ addFromPath })
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- 工具栏 -->
    <div class="flex items-center justify-between border-b p-4">
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          :disabled="isRunning || (stats.waiting === 0 && stats.failed === 0)"
          @click="startCapture"
        >
          <Play class="mr-2 size-4" />
          开始截图
        </Button>
        <Button variant="outline" size="sm" :disabled="!isRunning" @click="stopCapture">
          <Square class="mr-2 size-4" />
          停止
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button variant="outline" size="sm" :disabled="tasks.length === 0" class="gap-2">
              <Trash2 class="size-4" />
              删除
              <ChevronDown class="size-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" class="w-44">
            <DropdownMenuItem :disabled="stats.completed === 0" @click="clearCompleted">
              删除完成任务
            </DropdownMenuItem>
            <DropdownMenuItem :disabled="stats.failed === 0" @click="clearFailed">
              删除失败任务
            </DropdownMenuItem>
            <DropdownMenuItem :disabled="isRunning || tasks.length === 0" @click="clearAll">
              删除全部任务
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>

    <!-- 进度统计 -->
    <div v-if="tasks.length > 0" class="border-b px-4 py-3">
      <div class="mb-2 flex items-center justify-between">
        <div class="flex items-center gap-4">
          <span class="text-sm font-medium">任务统计</span>
          <div class="flex items-center gap-2 text-xs text-muted-foreground">
            <span>总计: {{ stats.total }}</span>
            <span class="text-blue-500">运行中: {{ stats.running }}</span>
            <span class="text-green-500">完成: {{ stats.completed }}</span>
            <span v-if="stats.failed > 0" class="text-destructive">失败: {{ stats.failed }}</span>
          </div>
        </div>
      </div>
      <Progress :model-value="progressPercent" class="h-2" />
    </div>

    <!-- 任务表格 -->
    <ScrollArea class="min-h-0 flex-1">
      <div v-if="tasks.length === 0" class="flex flex-1 items-center justify-center py-20 text-muted-foreground">
        <div class="text-center">
          <Camera class="mx-auto mb-4 size-12 opacity-30" />
          <p>暂无截图任务</p>
          <p class="mt-1 text-xs">请从目录管理页面右键添加</p>
        </div>
      </div>

      <Table v-else>
        <TableHeader>
          <TableRow>
            <TableHead class="w-[50%]">文件名</TableHead>
            <TableHead class="w-[30%]">错误信息</TableHead>
            <TableHead class="w-[20%] text-right">状态</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <ContextMenu v-for="task in tasks" :key="task.videoId">
            <ContextMenuTrigger as-child>
              <TableRow class="cursor-context-menu">
                <TableCell class="max-w-0 truncate text-sm" :title="task.videoPath">
                  {{ getFileName(task.videoPath) }}
                </TableCell>
                <TableCell class="max-w-0 truncate text-xs text-destructive" :title="task.error">
                  {{ task.error || '' }}
                </TableCell>
                <TableCell class="text-right">
                  <Badge :variant="getStatusVariant(task.status)">
                    {{ getStatusText(task.status) }}
                  </Badge>
                </TableCell>
              </TableRow>
            </ContextMenuTrigger>
            <ContextMenuContent>
              <ContextMenuItem @click="handlePlay(task)">
                <Play class="mr-2 size-4" />
                播放
              </ContextMenuItem>
              <ContextMenuItem @click="handleOpenFolder(task)">
                <FolderOpen class="mr-2 size-4" />
                打开目录
              </ContextMenuItem>
              <ContextMenuSeparator />
              <ContextMenuItem :disabled="task.status !== 'failed'" @click="handleRetry(task)">
                <RotateCcw class="mr-2 size-4" />
                重试
              </ContextMenuItem>
              <ContextMenuItem class="text-destructive" @click="handleDelete(task)">
                <Trash2 class="mr-2 size-4" />
                删除
              </ContextMenuItem>
            </ContextMenuContent>
          </ContextMenu>
        </TableBody>
      </Table>
    </ScrollArea>
  </div>
</template>
