<script setup lang="ts">
import { computed } from 'vue'
import { toast } from 'vue-sonner'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { ExternalLink } from 'lucide-vue-next'

interface Props {
  open: boolean
  toolName: string
  downloadUrl: string
}

const props = defineProps<Props>()

const emit = defineEmits<{
  'update:open': [value: boolean]
}>()

const isOpen = computed({
  get: () => props.open,
  set: (value) => emit('update:open', value)
})

// 打开官网
const handleOpenUrl = async () => {
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(props.downloadUrl)
  } catch {
    toast.error('打开失败')
  }
}
</script>

<template>
  <Dialog v-model:open="isOpen">
    <DialogContent class="sm:max-w-[460px]">
      <DialogTitle>工具未找到</DialogTitle>
      <DialogDescription>
        <span class="font-semibold text-foreground">{{ toolName }}</span> 未在系统中检测到。
        <br /><br />
        请前往官方页面下载安装后，回到设置页面通过「选择文件」指定可执行文件路径。
        <br /><br />
        <span class="text-xs text-muted-foreground break-all">{{ downloadUrl }}</span>
      </DialogDescription>
      <div class="flex items-center gap-3 mt-4">
        <Button variant="outline" size="sm" @click="handleOpenUrl">
          <ExternalLink class="w-4 h-4 mr-2" />
          打开官网
        </Button>
        <div class="flex-1" />
        <Button variant="default" @click="isOpen = false">
          关闭
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
