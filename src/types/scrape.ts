// 刮削相关类型定义

/** 刮削任务状态 */
export enum ScrapeStatus {
    /** 等待中 */
    Waiting = 'waiting',
    /** 进行中 */
    Running = 'running',
    /** 已完成 */
    Completed = 'completed',
    /** 部分完成 */
    PartialCompleted = 'partial',
    /** 失败 */
    Failed = 'failed',
}

/** 刮削任务步骤 */
export enum ScrapeStep {
    Pending = 0,
    VerifyCF = 1,
    FetchMeta = 2,
    DownloadCover = 3,
    SaveNFO = 4,
    UpdateDB = 5,
}

/** 刮削任务 */
export interface ScrapeTask {
    id: string
    path: string
    progress: number
    status: ScrapeStatus
    startedAt?: string
    completedAt?: string
}

/** 刮削日志条目 */
export interface ScrapeLogEntry {
    id: string
    taskId: string
    timestamp: string
    level: 'info' | 'success' | 'warning' | 'error'
    designation?: string
    message: string
    source?: string
}

/** 刮削器配置 */
export interface ScraperConfig {
    id: string
    name: string
    priority: number
    enabled: boolean
    requiresProxy: boolean
}
