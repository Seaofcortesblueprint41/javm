<script setup lang="ts">
import { ref } from 'vue'
import type { ResourceItem } from '@/types/resourceSearch'
import {
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import ResourceDetailDialog from '@/components/ResourceDetailDialog.vue'

const isDev = import.meta.env.DEV

// 组件属性
defineProps<{
  results: ResourceItem[]  // 搜索结果列表
  loading: boolean         // 是否正在加载
  searched: boolean        // 是否已执行过搜索
}>()

// 详情对话框状态
const detailOpen = ref(false)
const selectedResource = ref<ResourceItem | null>(null)

/** 缺失字段显示占位符 */
function displayValue(value: string): string {
  return value || '-'
}

function detailBadgeClass(level?: string): string {
  switch (level) {
    case '完整':
      return 'bg-emerald-500/15 text-emerald-300 ring-1 ring-emerald-500/30'
    case '丰富':
      return 'bg-sky-500/15 text-sky-300 ring-1 ring-sky-500/30'
    case '标准':
      return 'bg-amber-500/15 text-amber-300 ring-1 ring-amber-500/30'
    default:
      return 'bg-muted text-muted-foreground ring-1 ring-border'
  }
}

const emit = defineEmits<{
  'find-links': [code: string]
}>()

/** 点击结果行，打开详情对话框 */
function handleRowClick(item: ResourceItem) {
  selectedResource.value = item
  detailOpen.value = true
}
</script>

<template>
  <!-- 初始状态：未搜索过，不显示任何内容 -->
  <div v-if="!loading && !searched" />

  <!-- 加载状态 / 有结果 / 无结果 -->
  <Table v-else class="table-fixed">
    <TableHeader>
      <TableRow>
        <TableHead class="w-32 min-w-32">番号</TableHead>
        <TableHead class="max-w-0">名称</TableHead>
        <TableHead class="w-24 min-w-24 text-center">丰富度</TableHead>
        <TableHead class="w-32 min-w-32">演员</TableHead>
      </TableRow>
    </TableHeader>
    <TableBody>
      <!-- 已有结果：边加载边显示 -->
      <template v-if="results.length > 0">
        <TableRow
          v-for="item in results"
          :key="item.code + (item.source ?? '')"
          class="cursor-pointer hover:bg-muted/50"
          @click="handleRowClick(item)"
        >
          <TableCell class="w-32 min-w-32 whitespace-nowrap">{{ displayValue(item.code) }}</TableCell>
          <TableCell class="max-w-0">
            <div class="truncate">{{ displayValue(item.title) }}</div>
            <div v-if="isDev" class="mt-1 space-y-1 text-[11px] text-muted-foreground">
              <div class="truncate">来源: {{ displayValue(item.source || '') }}</div>
              <div class="truncate font-mono">链接: {{ displayValue(item.pageUrl || '') }}</div>
            </div>
          </TableCell>
          <TableCell class="w-24 min-w-24 text-center">
            <span
              class="inline-flex min-w-14 items-center justify-center rounded-full px-2 py-1 text-xs font-medium"
              :class="detailBadgeClass(item.detailLevel)"
            >
              {{ displayValue(item.detailLevel || '简略') }}
            </span>
          </TableCell>
          <TableCell class="w-32 min-w-32 truncate">{{ displayValue(item.actors) }}</TableCell>
        </TableRow>
        <!-- 加载中：在已有结果下方追加骨架行 -->
        <TableRow v-if="loading" v-for="i in 2" :key="`skeleton-tail-${i}`">
          <TableCell v-for="j in 4" :key="`skeleton-tail-${i}-${j}`">
            <div class="h-4 w-full animate-pulse rounded bg-muted" />
          </TableCell>
        </TableRow>
      </template>

      <!-- 加载中但还没有结果：骨架屏 -->
      <template v-else-if="loading">
        <TableRow v-for="i in 5" :key="`skeleton-${i}`">
          <TableCell v-for="j in 4" :key="`skeleton-${i}-${j}`">
            <div class="h-4 w-full animate-pulse rounded bg-muted" />
          </TableCell>
        </TableRow>
      </template>

      <!-- 搜索完成但无结果 -->
      <TableEmpty v-else-if="searched" :colspan="4">
        搜索失败，未找到相关资源
      </TableEmpty>
    </TableBody>
  </Table>

  <!-- 视频详情对话框 -->
  <ResourceDetailDialog
    v-model:open="detailOpen"
    :resource="selectedResource"
    @find-links="(code) => emit('find-links', code)"
  />
</template>
