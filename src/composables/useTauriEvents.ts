// Tauri 事件监听 Composable
import { ref, onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { DownloadProgress, ScrapeLogEntry } from '@/types'

/**
 * 监听下载进度事件
 */
export function useDownloadProgress() {
    const progress = ref<DownloadProgress | null>(null)
    let unlisten: UnlistenFn | null = null

    onMounted(async () => {
        const isTauri = typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
        if (!isTauri) return
        unlisten = await listen<DownloadProgress>('download-progress', (event) => {
            progress.value = event.payload
        })
    })

    onUnmounted(() => {
        unlisten?.()
    })

    return {
        progress,
    }
}

/**
 * 监听扫描进度事件
 */
export function useScanProgress() {
    const progress = ref<{
        current: number
        total: number
        current_file: string
    } | null>(null)
    let unlisten: UnlistenFn | null = null

    onMounted(async () => {
        const isTauri = typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
        if (!isTauri) return
        unlisten = await listen<typeof progress.value>('scan-progress', (event) => {
            progress.value = event.payload
        })
    })

    onUnmounted(() => {
        unlisten?.()
    })

    return {
        progress,
    }
}

/**
 * 监听刮削日志事件
 */
export function useScrapeLog() {
    const logs = ref<ScrapeLogEntry[]>([])
    const maxLogs = 100
    let unlisten: UnlistenFn | null = null

    onMounted(async () => {
        const isTauri = typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
        if (!isTauri) return
        unlisten = await listen<ScrapeLogEntry>('scrape-log', (event) => {
            logs.value.unshift(event.payload)
            // 保持日志数量在限制内
            if (logs.value.length > maxLogs) {
                logs.value = logs.value.slice(0, maxLogs)
            }
        })
    })

    onUnmounted(() => {
        unlisten?.()
    })

    const clearLogs = () => {
        logs.value = []
    }

    return {
        logs,
        clearLogs,
    }
}

/**
 * 监听刮削进度事件
 */
export function useScrapeProgress() {
    const progress = ref<{
        taskId: string
        total: number
        processed: number
        success: number
        failed: number
    } | null>(null)
    let unlisten: UnlistenFn | null = null

    onMounted(async () => {
        const isTauri = typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
        if (!isTauri) return
        unlisten = await listen<typeof progress.value>('scrape-progress', (event) => {
            progress.value = event.payload
        })
    })

    onUnmounted(() => {
        unlisten?.()
    })

    return {
        progress,
    }
}

/**
 * 监听刮削任务进度事件
 */
export function useScrapeTaskProgress() {
    const progress = ref<{
        task_id: string
        progress: number
    } | null>(null)
    let unlisten: UnlistenFn | null = null

    onMounted(async () => {
        const isTauri = typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
        if (!isTauri) return
        
        console.log('[useScrapeTaskProgress] Setting up listener for scrape-task-progress events')
        
        unlisten = await listen<typeof progress.value>('scrape-task-progress', (event) => {
            console.log('[useScrapeTaskProgress] Received event:', event.payload)
            progress.value = event.payload
        })
        
        console.log('[useScrapeTaskProgress] Listener registered successfully')
    })

    onUnmounted(() => {
        console.log('[useScrapeTaskProgress] Cleaning up listener')
        unlisten?.()
    })

    return {
        progress,
    }
}

/**
 * 监听任务队列状态事件
 */
export function useTaskQueueStatus() {
    const status = ref<{
        status: string
    } | null>(null)
    let unlisten: UnlistenFn | null = null

    onMounted(async () => {
        const isTauri = typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
        if (!isTauri) return
        unlisten = await listen<typeof status.value>('task-queue-status', (event) => {
            const nextStatus = event.payload?.status
            const currentStatus = status.value?.status
            if (nextStatus && nextStatus === currentStatus) return
            status.value = event.payload
        })
    })

    onUnmounted(() => {
        unlisten?.()
    })

    return {
        status,
    }
}

/**
 * 监听刮削任务失败事件
 */
export function useScrapeTaskFailed() {
    const failedTask = ref<{
        task_id: string
        error: string
    } | null>(null)
    let unlisten: UnlistenFn | null = null

    onMounted(async () => {
        const isTauri = typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)
        if (!isTauri) return
        unlisten = await listen<typeof failedTask.value>('scrape-task-failed', (event) => {
            // 每次收到新事件时，创建新的对象引用以触发 watch
            if (event.payload) {
                failedTask.value = { ...event.payload } as { task_id: string; error: string }
            }
        })
    })

    onUnmounted(() => {
        unlisten?.()
    })

    return failedTask
}
