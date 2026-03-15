<script setup lang="ts">
import { ref, computed } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { FolderOpen, Play, Square, Trash2 } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import type { ScrapeTask } from '@/types'
import { SCRAPE_STATUS_TEXT } from '@/utils/constants'

interface Props {
  tasks: ScrapeTask[]
}

const props = defineProps<Props>()

const emit = defineEmits<{
  (e: 'openFolder', task: ScrapeTask): void
  (e: 'startTask', task: ScrapeTask): void
  (e: 'stopTask', task: ScrapeTask): void
  (e: 'removeTask', task: ScrapeTask): void
}>()

// 容器引用
const containerRef = ref<HTMLElement>()

// 固定行高
const ROW_HEIGHT = 60

// 刮削步骤文本映射
const STEP_TEXT: Record<number, string> = {
  0: '等待中',
  1: '验证 Cloudflare',
  2: '获取元数据',
  3: '下载封面',
  4: '保存 .nfo 文件',
  5: '更新数据库',
}

const getStepText = (progress: number) => {
  return STEP_TEXT[progress] || '未知状态'
}

// 刮削状态颜色
const getStatusVariant = (status: string): 'default' | 'secondary' | 'destructive' | 'outline' => {
  switch (status) {
    case 'running': return 'default'
    case 'completed': return 'secondary'
    case 'failed': return 'destructive'
    default: return 'outline'
  }
}

// 虚拟化器
const virtualizer = useVirtualizer({
  get count() { return props.tasks.length },
  getScrollElement: () => containerRef.value ?? null,
  estimateSize: () => ROW_HEIGHT,
  overscan: 5,
})

// 虚拟行
const virtualRows = computed(() => virtualizer.value.getVirtualItems())

// 总高度
const totalHeight = computed(() => virtualizer.value.getTotalSize())
</script>

<template>
  <div ref="containerRef" class="flex-1 overflow-auto">
    <div :style="{ height: `${totalHeight}px`, position: 'relative' }">
      <div
        v-for="virtualRow in virtualRows"
        :key="String(virtualRow.key)"
        :style="{
          position: 'absolute',
          top: 0,
          left: 0,
          width: '100%',
          height: `${virtualRow.size}px`,
          transform: `translateY(${virtualRow.start}px)`,
        }"
      >
        <ContextMenu>
          <ContextMenuTrigger as-child>
            <div
              class="flex items-center border-b h-full hover:bg-muted/50 transition-colors cursor-context-menu"
            >
              <!-- 路径 -->
              <div class="flex-1 min-w-0 px-4 py-3">
                <div class="text-sm truncate" :title="tasks[virtualRow.index].path">
                  {{ tasks[virtualRow.index].path }}
                </div>
              </div>

              <!-- 进度 -->
              <div class="w-40 shrink-0 px-4 py-3">
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <div class="flex items-center gap-2 cursor-help">
                        <Progress
                          :model-value="tasks[virtualRow.index].status === 'completed' ? 100 : Math.min(tasks[virtualRow.index].progress * 20, 100)"
                          class="h-2 w-full"
                        />
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      <p>{{ getStepText(tasks[virtualRow.index].progress) }}</p>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>

              <!-- 状态 -->
              <div class="w-24 shrink-0 px-4 py-3">
                <Badge :variant="getStatusVariant(tasks[virtualRow.index].status)">
                  {{ SCRAPE_STATUS_TEXT[tasks[virtualRow.index].status] }}
                </Badge>
              </div>
            </div>
          </ContextMenuTrigger>

          <ContextMenuContent>
            <ContextMenuItem @click="emit('openFolder', tasks[virtualRow.index])">
              <FolderOpen class="mr-2 size-4" />
              打开所在目录
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem
              :disabled="tasks[virtualRow.index].status === 'running'"
              @click="emit('startTask', tasks[virtualRow.index])"
            >
              <Play class="mr-2 size-4" />
              开始任务
            </ContextMenuItem>
            <ContextMenuItem
              :disabled="tasks[virtualRow.index].status !== 'running'"
              @click="emit('stopTask', tasks[virtualRow.index])"
            >
              <Square class="mr-2 size-4" />
              停止任务
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem @click="emit('removeTask', tasks[virtualRow.index])">
              <Trash2 class="mr-2 size-4" />
              删除任务
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* shadcn 风格滚动条 */
.overflow-auto {
  scrollbar-width: thin;
  scrollbar-color: transparent transparent;
}

.overflow-auto:hover {
  scrollbar-color: hsl(0 0% 20%) transparent;
}

.overflow-auto::-webkit-scrollbar {
  width: 10px;
}

.overflow-auto::-webkit-scrollbar-track {
  background: transparent;
}

.overflow-auto::-webkit-scrollbar-thumb {
  background-color: transparent;
  border-radius: 9999px;
  border: 2px solid transparent;
  background-clip: content-box;
  transition: background-color 0.2s;
}

.overflow-auto:hover::-webkit-scrollbar-thumb {
  background-color: hsl(0 0% 20%);
}

.overflow-auto::-webkit-scrollbar-thumb:hover {
  background-color: hsl(0 0% 30%);
}
</style>
