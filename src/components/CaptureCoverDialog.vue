<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import {
    Dialog,
    DialogContent,
    DialogTitle,
    DialogDescription,
    DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Loader2, RefreshCw, Check, Square } from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

interface Props {
    open: boolean
    videoId: string
    videoPath: string
    mode?: 'single' | 'multiple'
}

const props = withDefaults(defineProps<Props>(), {
    mode: 'single'
})

const emit = defineEmits<{
    (e: 'update:open', value: boolean): void
    (e: 'success', coverPath: string | string[]): void
}>()

const isOpen = computed({
    get: () => props.open,
    set: (val) => emit('update:open', val),
})

const loading = ref(false)
const frames = ref<string[]>([])
const selectedIndex = ref<number | null>(null)
const selectedIndices = ref<Set<number>>(new Set())
const captureStatus = ref('')
const captureDone = ref(false) // 截图是否全部完成

// 事件监听器清理函数
let unlistenFrame: UnlistenFn | null = null
let unlistenDone: UnlistenFn | null = null

// 转换帧路径为可显示的 URL
const frameUrls = computed(() => {
    return frames.value.map(path => convertFileSrc(path.replace(/\\/g, '/')))
})

// 设置事件监听
const setupListeners = async () => {
    // 清理旧的监听器
    await cleanupListeners()

    // 监听每一帧截图完成
    unlistenFrame = await listen<string>('capture-frame-ready', (event) => {
        frames.value.push(event.payload)
        // 多选模式下自动选中新帧
        if (props.mode === 'multiple') {
            selectedIndices.value.add(frames.value.length - 1)
            selectedIndices.value = new Set(selectedIndices.value)
        }
    })

    // 监听截图全部完成
    unlistenDone = await listen('capture-done', () => {
        captureDone.value = true
        loading.value = false
        captureStatus.value = ''
        if (frames.value.length === 0) {
            toast.error('未能截取到视频帧')
        }
    })
}

// 清理事件监听
const cleanupListeners = async () => {
    if (unlistenFrame) {
        unlistenFrame()
        unlistenFrame = null
    }
    if (unlistenDone) {
        unlistenDone()
        unlistenDone = null
    }
}

// 截取视频帧
const captureFrames = async () => {
    if (!props.videoPath) return

    loading.value = true
    captureDone.value = false
    frames.value = []
    selectedIndex.value = null
    selectedIndices.value = new Set()
    captureStatus.value = '正在截取视频帧...'

    // 先设置监听器，再发起截图
    await setupListeners()

    try {
        // invoke 会在所有帧截完后才返回，但帧会通过事件实时推送
        await invoke<string[]>('capture_video_frames', {
            videoPath: props.videoPath,
            count: 10
        })
    } catch (e) {
        const errMsg = String(e)
        console.error('截取视频帧失败:', errMsg)

        if (errMsg.includes('已取消') || errMsg.includes('被取消')) {
            // 取消不算错误，帧已经通过事件实时显示了
            if (frames.value.length > 0) {
                toast.info('截图已停止')
            }
        } else if (errMsg.includes('损坏')) {
            toast.error('视频文件可能已损坏，无法截图')
        } else if (errMsg.includes('ffmpeg')) {
            toast.error('请确保系统已安装 ffmpeg')
        } else {
            toast.error('截取失败: ' + errMsg)
        }
    } finally {
        loading.value = false
        captureDone.value = true
        captureStatus.value = ''
    }
}

// 停止截图
const stopCapture = async () => {
    try {
        await invoke('cancel_capture')
        captureStatus.value = '正在停止...'
    } catch (e) {
        console.error('停止截图失败:', e)
    }
}

// 选择帧
const selectFrame = (index: number) => {
    if (props.mode === 'single') {
        selectedIndex.value = index
    } else {
        if (selectedIndices.value.has(index)) {
            selectedIndices.value.delete(index)
        } else {
            selectedIndices.value.add(index)
        }
        selectedIndices.value = new Set(selectedIndices.value)
    }
}

// 判断帧是否被选中
const isFrameSelected = (index: number) => {
    if (props.mode === 'single') {
        return selectedIndex.value === index
    } else {
        return selectedIndices.value.has(index)
    }
}

// 确认选择（同时停止后台截图）
const confirmSelection = async () => {
    if (props.mode === 'single') {
        if (selectedIndex.value === null) {
            toast.error('请先选择一个封面')
            return
        }

        // 停止后台还在进行的截图
        if (!captureDone.value) {
            await stopCapture()
        }

        loading.value = true

        try {
            const selectedFrame = frames.value[selectedIndex.value]
            const coverPath = await invoke<string>('save_captured_cover', {
                videoId: props.videoId,
                videoPath: props.videoPath,
                framePath: selectedFrame
            })

            toast.success('封面已保存')
            emit('success', coverPath)
            isOpen.value = false
        } catch (e) {
            console.error('保存封面失败:', e)
            toast.error('保存失败: ' + String(e))
        } finally {
            loading.value = false
        }
    } else {
        if (selectedIndices.value.size === 0) {
            toast.error('请至少选择一张预览图')
            return
        }

        // 停止后台还在进行的截图
        if (!captureDone.value) {
            await stopCapture()
        }

        loading.value = true

        try {
            const selectedFrames = Array.from(selectedIndices.value).map(idx => frames.value[idx])
            const screenshotPaths = await invoke<string[]>('save_captured_screenshots', {
                videoId: props.videoId,
                videoPath: props.videoPath,
                framePaths: selectedFrames
            })

            toast.success(`已保存 ${screenshotPaths.length} 张预览图`)
            emit('success', screenshotPaths)
            isOpen.value = false
        } catch (e) {
            console.error('保存预览图失败:', e)
            toast.error('保存失败: ' + String(e))
        } finally {
            loading.value = false
        }
    }
}

// 对话框关闭时取消截图并清空数据
const handleOpenChange = (open: boolean) => {
    isOpen.value = open
    if (!open) {
        if (loading.value || !captureDone.value) {
            stopCapture()
        }
        cleanupListeners()
        frames.value = []
        selectedIndex.value = null
        selectedIndices.value = new Set()
        captureStatus.value = ''
        captureDone.value = false
    }
}

// 监听 props.open 变化，自动获取视频帧
watch(() => props.open, (newOpen) => {
    if (newOpen && props.videoPath) {
        captureFrames()
    }
}, { immediate: false })

// 组件卸载时清理
onUnmounted(() => {
    cleanupListeners()
})
</script>

<template>
    <Dialog :open="isOpen" @update:open="handleOpenChange">
        <DialogContent class="sm:max-w-[800px] h-[80vh] flex flex-col p-0 gap-0">
            <div class="p-6 pb-4 border-b">
                <DialogTitle>{{ props.mode === 'single' ? '截取封面' : '截取预览图' }}</DialogTitle>
                <DialogDescription>
                    {{ props.mode === 'single' ? '从视频中截取帧，选择一个作为封面' : '从视频中截取帧，可以选择多个作为预览图' }}
                </DialogDescription>
            </div>

            <div class="flex-1 min-h-0 p-6">
                <!-- 加载中且还没有帧 -->
                <div v-if="loading && frames.length === 0" class="flex flex-col items-center justify-center h-full">
                    <Loader2 class="size-12 animate-spin text-muted-foreground mb-4" />
                    <p class="text-sm text-muted-foreground">{{ captureStatus || '正在截取视频帧...' }}</p>
                </div>

                <!-- 帧网格（有帧就显示，即使还在截图中） -->
                <ScrollArea v-else-if="frames.length > 0" class="h-full">
                    <div class="grid grid-cols-2 gap-4">
                        <div
                            v-for="(frameUrl, index) in frameUrls"
                            :key="index"
                            class="group relative aspect-video rounded-lg overflow-hidden border-2 cursor-pointer transition-all bg-black/5"
                            :class="isFrameSelected(index) ? 'border-primary ring-2 ring-primary' : 'border-border'"
                            @click="selectFrame(index)"
                        >
                            <img
                                :src="frameUrl"
                                class="w-full h-full object-contain"
                                alt="视频帧"
                            />
                            <div
                                v-if="isFrameSelected(index)"
                                class="absolute top-2 left-2 size-7 rounded-full bg-primary flex items-center justify-center shadow-md"
                            >
                                <Check class="size-4 text-primary-foreground" />
                            </div>
                            <div
                                v-else
                                class="absolute top-2 left-2 size-7 rounded-full border-2 border-white/70 bg-black/30 opacity-0 group-hover:opacity-100 transition-opacity"
                            />
                            <div class="absolute bottom-2 right-2 bg-black/70 text-white text-xs px-2 py-1 rounded">
                                {{ index + 1 }}
                            </div>
                        </div>
                        <!-- 截图进行中的占位提示 -->
                        <div
                            v-if="!captureDone"
                            class="aspect-video rounded-lg border-2 border-dashed border-border flex items-center justify-center"
                        >
                            <div class="flex flex-col items-center text-muted-foreground">
                                <Loader2 class="size-6 animate-spin mb-2" />
                                <span class="text-xs">截图中...</span>
                            </div>
                        </div>
                    </div>
                </ScrollArea>

                <!-- 空状态 -->
                <div v-else class="flex flex-col items-center justify-center h-full text-muted-foreground">
                    <p class="text-sm">未能截取到视频帧</p>
                </div>
            </div>

            <DialogFooter class="p-6 pt-4 border-t">
                <Button
                    v-if="!captureDone"
                    variant="destructive"
                    @click="stopCapture"
                >
                    <Square class="mr-2 size-4" />
                    停止截图
                </Button>
                <Button
                    variant="outline"
                    @click="captureFrames"
                    :disabled="loading"
                >
                    <RefreshCw class="mr-2 size-4" />
                    再次获取
                </Button>
                <Button
                    @click="confirmSelection"
                    :disabled="props.mode === 'single' ? selectedIndex === null : selectedIndices.size === 0"
                >
                    确认{{ props.mode === 'multiple' && selectedIndices.size > 0 ? ` (${selectedIndices.size})` : '' }}
                </Button>
                <Button
                    variant="outline"
                    @click="isOpen = false"
                >
                    关闭
                </Button>
            </DialogFooter>
        </DialogContent>
    </Dialog>
</template>
