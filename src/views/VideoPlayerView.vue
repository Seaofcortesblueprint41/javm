<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useSettingsStore } from '@/stores/settings'
import Plyr from 'plyr'
import 'plyr/dist/plyr.css'
import Hls from 'hls.js'
import WindowControls from '@/components/layout/WindowControls.vue'
import { Button } from '@/components/ui/button'

const route = useRoute()
const videoElement = ref<HTMLVideoElement | null>(null)
const windowControlsRef = ref<InstanceType<typeof WindowControls> | null>(null)
const settingsStore = useSettingsStore()

let player: Plyr | null = null
let hls: Hls | null = null
let syntheticM3u8Url: string | null = null
let unlistenResize: (() => void) | null = null
let unlistenMove: (() => void) | null = null
let saveTimeout: ReturnType<typeof setTimeout> | null = null

const saveWindowPosition = () => {
    if (saveTimeout) clearTimeout(saveTimeout)
    saveTimeout = setTimeout(async () => {
        const win = getCurrentWindow()
        if (win.label.startsWith('video_player_')) {
            const size = await win.outerSize()
            const pos = await win.outerPosition()

            settingsStore.updateSettings({
                videoPlayer: {
                    width: size.width,
                    height: size.height,
                    x: pos.x,
                    y: pos.y,
                    alwaysOnTop: settingsStore.settings.videoPlayer?.alwaysOnTop ?? false
                }
            })
        }
    }, 500) // 延迟保存避免频繁写入
}

const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 't' || e.key === 'T') {
        const target = e.target as HTMLElement
        if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return
        windowControlsRef.value?.toggleAlwaysOnTop()
        e.preventDefault()
        e.stopPropagation()
    }
}

const videoUrl = ref('')
const videoTitle = ref('')
const isHls = ref(false)
const isTsFile = ref(false)
const originalUrl = ref('')
const playbackError = ref('')

onMounted(async () => {
    document.addEventListener('keydown', handleKeyDown, true)
    const queryUrl = route.query.url as string || ''
    const queryTitle = route.query.title as string || 'Unknown Video'
    const queryIsHls = route.query.is_hls === 'true'

    videoTitle.value = decodeURIComponent(queryTitle)
    document.title = videoTitle.value
    originalUrl.value = decodeURIComponent(queryUrl)

    const decodedUrl = decodeURIComponent(queryUrl)
    if (queryIsHls || queryUrl.startsWith('http://') || queryUrl.startsWith('https://')) {
        videoUrl.value = decodedUrl
    } else {
        videoUrl.value = convertFileSrc(decodedUrl)
    }

    isHls.value = queryIsHls
    isTsFile.value = /\.(m2ts|ts)$/i.test(decodedUrl)

    if (videoElement.value) {
        initPlayer()
    }

    // 监听窗口大小和位置改变
    const win = getCurrentWindow()
    if (win.label.startsWith('video_player_')) {
        unlistenResize = await win.onResized(() => saveWindowPosition())
        unlistenMove = await win.onMoved(() => saveWindowPosition())
    }
})

/**
 * 创建 HLS.js 自定义 Loader 工厂函数
 * 通过 Tauri 后端代理所有 HTTP 请求来绕过 CORS
 */
function createProxyLoaderClass(): any {
    return class TauriProxyLoader {
        context: any = null
        stats: any
        private aborted = false

        constructor(_config: any) {
            this.stats = {
                aborted: false,
                loaded: 0,
                retry: 0,
                total: 0,
                chunkCount: 0,
                bwEstimate: 0,
                loading: { start: 0, first: 0, end: 0 },
                parsing: { start: 0, end: 0 },
                buffering: { start: 0, first: 0, end: 0 },
            }
        }

        destroy() {
            this.aborted = true
            this.context = null
        }

        abort() {
            this.aborted = true
            this.stats.aborted = true
        }

        load(context: any, _config: any, callbacks: any) {
            this.context = context
            this.aborted = false

            const url = context.url
            const startTime = performance.now()
            this.stats.loading.start = startTime

            // 推断 Referer
            let referer: string | null = null
            try {
                const u = new URL(url)
                referer = u.origin + '/'
            } catch { /* ignore */ }

            invoke<[string, string]>('proxy_hls_request', { url, referer })
                .then(([b64Data, contentType]) => {
                    if (this.aborted) return

                    // base64 解码
                    const binary = atob(b64Data)
                    const bytes = new Uint8Array(binary.length)
                    for (let i = 0; i < binary.length; i++) {
                        bytes[i] = binary.charCodeAt(i)
                    }

                    const now = performance.now()
                    this.stats.loading.first = now
                    this.stats.loading.end = now
                    this.stats.loaded = bytes.length
                    this.stats.total = bytes.length

                    // 根据 responseType 决定返回数据格式
                    // playlist 需要 string，fragment 需要 ArrayBuffer
                    const isPlaylist = contentType.includes('mpegurl') ||
                        contentType.includes('text') ||
                        url.endsWith('.m3u8') ||
                        context.responseType === 'text'

                    const response: any = {
                        url,
                        data: isPlaylist
                            ? new TextDecoder().decode(bytes)
                            : bytes.buffer,
                    }

                    callbacks.onSuccess(response, this.stats, context, null)
                })
                .catch((err: any) => {
                    if (this.aborted) return
                    callbacks.onError(
                        { code: 0, text: String(err) },
                        context,
                        null,
                        this.stats,
                    )
                })
        }

        getCacheAge() {
            return null
        }

        getResponseHeader() {
            return null
        }
    }
}

const createPlyrPlayer = (options?: Plyr.Options) => {
    if (!videoElement.value) return

    player = new Plyr(videoElement.value, options)
}

const destroyPlaybackEngines = () => {
    if (player) {
        player.destroy()
        player = null
    }

    if (hls) {
        hls.destroy()
        hls = null
    }

    if (syntheticM3u8Url) {
        URL.revokeObjectURL(syntheticM3u8Url)
        syntheticM3u8Url = null
    }
}

const initPlayer = () => {
    if (!videoElement.value) return

    destroyPlaybackEngines()
    playbackError.value = ''

    const defaultOptions: Plyr.Options = {
        autoplay: true,
        keyboard: { focused: true, global: true },
        seekTime: 5,
        controls: [
            'play-large',
            'play',
            'progress',
            'current-time',
            'duration',
            'mute',
            'volume',
            'captions',
            'settings',
            'pip',
            'airplay',
            'fullscreen',
        ],
        settings: ['captions', 'quality', 'speed', 'loop'],
        speed: { selected: 1, options: [0.5, 0.75, 1, 1.25, 1.5, 2] },
    }

    if (isHls.value && Hls.isSupported()) {
        const ProxyLoader = createProxyLoaderClass()
        hls = new Hls({
            // 使用自定义 Loader 通过后端代理绕过 CORS
            loader: ProxyLoader as any,
        })

        hls.loadSource(videoUrl.value)
        hls.attachMedia(videoElement.value)

        hls.on(Hls.Events.MANIFEST_PARSED, () => {
            const availableQualities = hls!.levels.map(l => l.height)
            const options: Plyr.Options = {
                ...defaultOptions,
                quality: {
                    default: availableQualities[0],
                    options: availableQualities,
                    forced: true,
                    onChange: (e: number) => {
                        const levelIndex = hls!.levels.findIndex(l => l.height === e)
                        if (levelIndex > -1) {
                            hls!.currentLevel = levelIndex
                        }
                    },
                },
            }
            createPlyrPlayer(options)
        })
    } else if (isTsFile.value && Hls.isSupported()) {
        // 使用 hls.js 播放本地 .ts 文件：构造虚拟 m3u8 播放列表
        const m3u8Content = [
            '#EXTM3U',
            '#EXT-X-VERSION:3',
            '#EXT-X-TARGETDURATION:99999',
            '#EXT-X-MEDIA-SEQUENCE:0',
            '#EXTINF:99999,',
            videoUrl.value,
            '#EXT-X-ENDLIST',
        ].join('\n')
        const blob = new Blob([m3u8Content], { type: 'application/vnd.apple.mpegurl' })
        syntheticM3u8Url = URL.createObjectURL(blob)

        hls = new Hls()
        hls.loadSource(syntheticM3u8Url)
        hls.attachMedia(videoElement.value)

        hls.on(Hls.Events.MANIFEST_PARSED, () => {
            createPlyrPlayer(defaultOptions)
        })

        hls.on(Hls.Events.ERROR, (_event, data) => {
            if (data.fatal) {
                console.error('HLS fatal error for TS file:', data)
                playbackError.value = '播放 TS 文件失败，请尝试使用系统播放器'
            }
        })
    } else {
        videoElement.value.src = videoUrl.value
        createPlyrPlayer(defaultOptions)
    }
}

const openInExternalPlayer = async () => {
    try {
        await invoke('open_with_player', { path: originalUrl.value })
    } catch (e) {
        console.error('Failed to open external player:', e)
    }
}

onUnmounted(() => {
    document.removeEventListener('keydown', handleKeyDown, true)
    destroyPlaybackEngines()
    if (unlistenResize) {
        unlistenResize()
    }
    if (unlistenMove) {
        unlistenMove()
    }
    if (saveTimeout) {
        clearTimeout(saveTimeout)
    }
})
</script>

<template>
    <div class="relative w-full h-full bg-black flex flex-col group">
        <!-- 自定义顶部栏 (绝对定位，悬浮在视频上方) -->
        <!-- 使用 group-hover:opacity-100 让其在鼠标移入时显示 -->
        <div data-tauri-drag-region
            class="absolute top-0 left-0 right-0 h-10 bg-gradient-to-b from-black/80 to-transparent z-50 flex items-center pl-4 opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-auto">
            <!-- 标题 -->
            <div class="flex-1 text-white text-sm truncate select-none pointer-events-none">
                {{ videoTitle }}
            </div>
            <!-- 窗口控制 -->
            <div class="flex-shrink-0 h-full" data-tauri-drag-region="false">
                <WindowControls ref="windowControlsRef" class="text-white hover:text-white" />
            </div>
        </div>

        <!-- 播放器容器 -->
        <div class="flex-1 w-full relative">
            <div v-if="playbackError" class="absolute inset-0 flex flex-col items-center justify-center bg-black text-white z-10">
                <p class="mb-4 text-lg">{{ playbackError }}</p>
                <Button @click="openInExternalPlayer" variant="secondary">
                    系统默认播放器播放
                </Button>
            </div>
            <video ref="videoElement" class="absolute inset-0 w-full h-full" crossorigin="anonymous" playsinline
                controls autoplay></video>
        </div>
    </div>
</template>

<style>
/* 确保 Plyr 容器撑满父级高度 */
.plyr,
.plyr__video-wrapper {
    height: 100% !important;
}

/* 覆盖 WindowControls 在暗色背景下的样式 */
.group .text-white button:hover {
    background-color: rgba(255, 255, 255, 0.2);
}

.group .text-white button.hover\:bg-red-500:hover {
    background-color: rgb(239 68 68);
}
</style>
