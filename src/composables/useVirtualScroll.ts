// 虚拟滚动 Composable
import { computed, type Ref } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { useWindowSize } from '@vueuse/core'

interface UseVirtualGridOptions<T> {
    items: Ref<T[]>
    containerRef: Ref<HTMLElement | undefined>
    itemHeight?: number
    gap?: number
    minColumns?: number
    maxColumns?: number
    columnWidth?: number
}

/**
 * 虚拟滚动网格 Composable
 */
export function useVirtualGrid<T>({
    items,
    containerRef,
    itemHeight = 320,
    gap = 16,
    minColumns = 2,
    maxColumns = 8,
    columnWidth = 200,
}: UseVirtualGridOptions<T>) {
    const { width: windowWidth } = useWindowSize()

    // 计算响应式列数
    const columns = computed(() => {
        const containerWidth = containerRef.value?.clientWidth || windowWidth.value - 280 // 减去侧边栏宽度
        const availableWidth = containerWidth - gap * 2
        const cols = Math.floor((availableWidth + gap) / (columnWidth + gap))
        return Math.max(minColumns, Math.min(maxColumns, cols))
    })

    // 计算行数
    const rowCount = computed(() => {
        return Math.ceil(items.value.length / columns.value)
    })

    // 虚拟化器
    const virtualizer = useVirtualizer({
        get count() { return rowCount.value },
        getScrollElement: () => containerRef.value ?? null,
        estimateSize: () => itemHeight + gap,
        overscan: 3,
    })

    // 获取某一行的项目
    const getRowItems = (rowIndex: number): T[] => {
        const start = rowIndex * columns.value
        const end = start + columns.value
        return items.value.slice(start, end)
    }

    // 虚拟项目
    const virtualRows = computed(() => virtualizer.value.getVirtualItems())

    // 总高度
    const totalHeight = computed(() => virtualizer.value.getTotalSize())

    return {
        columns,
        rowCount,
        virtualRows,
        totalHeight,
        getRowItems,
        virtualizer,
    }
}
