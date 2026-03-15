<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { FolderOpen, AlertCircle, CheckCircle2, XCircle, Trash2 } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { selectDirectory } from '@/lib/tauri'
import { toast } from 'vue-sonner'

interface Props {
  open: boolean
  defaultPath?: string
}

interface Emits {
  (e: 'update:open', value: boolean): void
  (e: 'submit', data: { tasks: Array<{ url: string, filename: string }>, savePath: string, setAsDefault: boolean }): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

// 表单数据
const rawInput = ref('')
const savePath = ref('')
const setAsDefault = ref(false)

// 解析后的链接列表
interface ParsedUrl {
  original: string
  cleaned: string
  filename: string
  valid: boolean
  error?: string
}

const parsedUrls = ref<ParsedUrl[]>([])

// 监听 defaultPath 变化
watch(() => props.defaultPath, (newPath) => {
  if (newPath && !savePath.value) {
    savePath.value = newPath
  }
}, { immediate: true })

// 监听弹框打开状态，重置表单
watch(() => props.open, (isOpen) => {
  if (isOpen) {
    rawInput.value = ''
    parsedUrls.value = []
    setAsDefault.value = false
    // 设置保存路径为默认路径
    if (props.defaultPath) {
      savePath.value = props.defaultPath
    }
  }
})

// URL 验证正则
const URL_PATTERN = /^https?:\/\/.+/i

// 清理和提取链接
function cleanAndExtractUrl(line: string): { url: string, extraText: string } {
  // 移除首尾空白
  let cleaned = line.trim()

  // 移除常见的包裹字符
  cleaned = cleaned.replace(/^["'`<>[\](){}]+|["'`<>[\](){}]+$/g, '')

  // 再次移除空白
  cleaned = cleaned.trim()

  // 尝试提取 URL（如果文本中包含 URL）
  const urlMatch = cleaned.match(/(https?:\/\/[^\s"'<>]+)/i)
  if (urlMatch && urlMatch.index !== undefined) {
    const url = urlMatch[1]
    // 提取 URL 前后的文本作为额外信息
    const beforeUrl = cleaned.substring(0, urlMatch.index).trim()
    const afterUrl = cleaned.substring(urlMatch.index + url.length).trim()
    const extraText = [beforeUrl, afterUrl].filter(t => t).join(' ')

    return { url, extraText }
  }

  return { url: cleaned, extraText: '' }
}

// 过滤文件名中的非法字符
function sanitizeFilename(filename: string): string {
  // Windows 和 Unix 系统都不允许的字符
  const illegalChars = /[<>:"/\\|?*\x00-\x1f]/g

  // 替换非法字符为下划线
  let sanitized = filename.replace(illegalChars, '_')

  // 移除首尾的点和空格（Windows 不允许）
  sanitized = sanitized.replace(/^[.\s]+|[.\s]+$/g, '')

  // 如果文件名为空或只有下划线，返回默认名称
  if (!sanitized || /^_+$/.test(sanitized)) {
    return 'video'
  }

  // 限制文件名长度（不包括扩展名）
  const maxLength = 200
  if (sanitized.length > maxLength) {
    sanitized = sanitized.substring(0, maxLength)
  }

  return sanitized
}

// 从 URL 中提取文件名
function extractFilenameFromUrl(url: string, extraText: string): string {
  // 如果有额外文本，优先使用额外文本作为文件名
  if (extraText) {
    return sanitizeFilename(extraText)
  }

  try {
    // 解析 URL
    const urlObj = new URL(url)
    const pathname = urlObj.pathname

    // 获取路径的最后一部分
    const segments = pathname.split('/').filter(s => s)
    if (segments.length === 0) {
      return 'video'
    }

    const lastSegment = segments[segments.length - 1]

    // 移除扩展名
    const filename = lastSegment.replace(/\.(m3u8|mp4|ts|flv|mkv|avi)$/i, '')

    return sanitizeFilename(filename) || 'video'
  } catch (e) {
    // URL 解析失败，使用默认名称
    return 'video'
  }
}

// 验证 URL
function validateUrl(url: string): { valid: boolean, error?: string } {
  if (!url) {
    return { valid: false, error: '链接为空' }
  }

  if (!URL_PATTERN.test(url)) {
    return { valid: false, error: '不是有效的 HTTP/HTTPS 链接' }
  }

  // 检查是否是常见的视频流格式
  const validExtensions = ['.m3u8', '.mp4', '.ts', '.flv', '.mkv', '.avi']
  const hasValidExtension = validExtensions.some(ext => url.toLowerCase().includes(ext))

  if (!hasValidExtension) {
    return { valid: false, error: '不是支持的视频格式链接' }
  }

  return { valid: true }
}

// 解析输入的链接
function parseUrls() {
  const lines = rawInput.value.split('\n')
  const results: ParsedUrl[] = []
  const seenUrls = new Set<string>()
  const seenFilenames = new Map<string, number>() // 用于处理重复文件名

  for (const line of lines) {
    // 跳过空行
    if (!line.trim()) {
      continue
    }

    const { url, extraText } = cleanAndExtractUrl(line)

    // 跳过清理后为空的行
    if (!url) {
      continue
    }

    // 检查重复
    if (seenUrls.has(url)) {
      results.push({
        original: line,
        cleaned: url,
        filename: '',
        valid: false,
        error: '重复的链接'
      })
      continue
    }

    seenUrls.add(url)

    // 验证 URL
    const validation = validateUrl(url)

    // 生成文件名
    let filename = extractFilenameFromUrl(url, extraText)

    // 处理重复文件名
    if (seenFilenames.has(filename)) {
      const count = seenFilenames.get(filename)! + 1
      seenFilenames.set(filename, count)
      filename = `${filename}_${count}`
    } else {
      seenFilenames.set(filename, 1)
    }

    results.push({
      original: line,
      cleaned: url,
      filename,
      valid: validation.valid,
      error: validation.error
    })
  }

  parsedUrls.value = results
}

// 监听输入变化，自动解析
watch(rawInput, () => {
  parseUrls()
})

// 统计信息
const stats = computed(() => ({
  total: parsedUrls.value.length,
  valid: parsedUrls.value.filter(u => u.valid).length,
  invalid: parsedUrls.value.filter(u => !u.valid).length
}))

// 是否可以提交
const canSubmit = computed(() => {
  return stats.value.valid > 0 && savePath.value.trim() !== ''
})

// 选择保存路径
async function handleSelectPath() {
  try {
    const selected = await selectDirectory()
    if (selected) {
      savePath.value = selected
    }
  } catch (e) {
    console.error('Failed to select directory:', e)
    toast.error('选择目录失败')
  }
}

// 移除无效链接
function removeInvalidUrls() {
  const validLines = parsedUrls.value
    .filter(u => u.valid)
    .map(u => u.original)
  rawInput.value = validLines.join('\n')
}

// 提交
function handleSubmit() {
  if (!canSubmit.value) {
    return
  }

  const validTasks = parsedUrls.value
    .filter(u => u.valid)
    .map(u => ({
      url: u.cleaned,
      filename: u.filename
    }))

  emit('submit', {
    tasks: validTasks,
    savePath: savePath.value.trim(),
    setAsDefault: setAsDefault.value
  })
}

// 关闭对话框
function handleClose() {
  emit('update:open', false)
}
</script>

<template>
  <Dialog :open="open" @update:open="handleClose">
    <DialogContent class="max-w-3xl max-h-[90vh] flex flex-col">
      <DialogHeader>
        <DialogTitle>添加下载任务</DialogTitle>
        <DialogDescription>
          每行一个下载链接，支持自动提取和验证
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-4 overflow-y-auto">
        <!-- 链接输入区 -->
        <div class="space-y-2">
          <div class="flex items-center justify-between">
            <Label for="urls">下载链接</Label>
            <div class="flex items-center gap-2">
              <Badge v-if="stats.total > 0" variant="outline">
                总计: {{ stats.total }}
              </Badge>
              <Badge v-if="stats.valid > 0" variant="default">
                <CheckCircle2 class="mr-1 size-3" />
                有效: {{ stats.valid }}
              </Badge>
              <Badge v-if="stats.invalid > 0" variant="destructive">
                <XCircle class="mr-1 size-3" />
                无效: {{ stats.invalid }}
              </Badge>
              <Button v-if="stats.invalid > 0" variant="ghost" size="sm" @click="removeInvalidUrls">
                <Trash2 class="mr-1 size-3" />
                移除无效
              </Button>
            </div>
          </div>
          <Textarea id="urls" v-model="rawInput"
            placeholder="粘贴下载链接，每行一个&#10;支持格式：.m3u8, .mp4, .ts, .flv, .mkv, .avi&#10;&#10;示例：&#10;https://example.com/video.m3u8&#10;&quot;https://example.com/movie.mp4&quot;&#10;<https://example.com/stream.m3u8>"
            class="min-h-[120px] font-mono text-sm" />
        </div>

        <!-- 解析结果预览 -->
        <div v-if="parsedUrls.length > 0" class="space-y-2">
          <Label>解析结果</Label>
          <ScrollArea class="h-[200px] rounded-md border">
            <div class="p-2 space-y-1">
              <div v-for="(url, index) in parsedUrls" :key="index" class="flex items-start gap-2 rounded p-2 text-sm"
                :class="url.valid ? 'bg-green-50 dark:bg-green-950/20' : 'bg-red-50 dark:bg-red-950/20'">
                <component :is="url.valid ? CheckCircle2 : XCircle"
                  :class="url.valid ? 'text-green-600' : 'text-red-600'" class="size-4 shrink-0 mt-0.5" />
                <div class="flex-1 min-w-0">
                  <div v-if="url.valid && url.filename" class="text-xs font-medium mb-1">
                    文件名: {{ url.filename }}
                  </div>
                  <div class="font-mono text-xs break-all text-muted-foreground">
                    {{ url.cleaned }}
                  </div>
                  <div v-if="!url.valid && url.error" class="text-xs text-red-600 dark:text-red-400 mt-1">
                    {{ url.error }}
                  </div>
                </div>
              </div>
            </div>
          </ScrollArea>
        </div>

        <!-- 保存路径 -->
        <div class="space-y-2">
          <Label for="path">保存路径</Label>
          <div class="flex gap-2">
            <Input id="path" v-model="savePath" placeholder="选择保存目录" readonly class="flex-1" />
            <Button variant="outline" @click="handleSelectPath">
              <FolderOpen class="size-4" />
            </Button>
          </div>
        </div>

        <!-- 设置为默认路径 -->
        <div class="flex items-center space-x-2">
          <Checkbox id="set-default" v-model="setAsDefault" />
          <Label for="set-default" class="text-sm font-normal cursor-pointer">
            将此目录设置为默认下载路径
          </Label>
        </div>

        <!-- 提示信息 -->
        <div v-if="stats.invalid > 0"
          class="flex items-start gap-2 rounded-md bg-yellow-50 dark:bg-yellow-950/20 p-3 text-sm">
          <AlertCircle class="size-4 text-yellow-600 shrink-0 mt-0.5" />
          <div class="text-yellow-800 dark:text-yellow-200">
            检测到 {{ stats.invalid }} 个无效链接，这些链接将不会被添加到下载队列
          </div>
        </div>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="handleClose">
          取消
        </Button>
        <Button :disabled="!canSubmit" @click="handleSubmit">
          添加 {{ stats.valid }} 个任务
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
