export interface AppUpdateInfo {
  configured: boolean
  available: boolean
  currentVersion: string
  version: string | null
  body: string | null
  date: string | null
  target: string | null
}

export interface AppUpdateProgress {
  phase: 'downloading' | 'installing'
  downloadedBytes: number
  totalBytes: number | null
  percentage: number | null
}