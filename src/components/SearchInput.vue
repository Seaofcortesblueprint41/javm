<script setup lang="ts">
import { ref } from 'vue'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Search, Loader2 } from 'lucide-vue-next'

// Props 定义
const props = defineProps<{
  loading: boolean // 是否正在搜索
}>()

// 事件定义
const emit = defineEmits<{
  (e: 'search', keyword: string): void // 触发搜索
}>()

// 输入框绑定值
const keyword = ref('')

/** 触发搜索，空白输入时阻止 */
function handleSearch() {
  const trimmed = keyword.value.trim()
  if (!trimmed) return
  emit('search', trimmed)
}

/** 回车触发搜索 */
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    handleSearch()
  }
}
</script>

<template>
  <div class="relative w-full">
    <Input
      v-model="keyword"
      placeholder="请输入番号（需要科学上网）..."
      class="pr-10"
      @keydown="handleKeydown"
    />
    <Button
      variant="ghost"
      size="icon-sm"
      class="absolute right-1 top-1/2 -translate-y-1/2"
      :disabled="props.loading"
      @click="handleSearch"
    >
      <Loader2 v-if="props.loading" class="animate-spin" />
      <Search v-else />
    </Button>
  </div>
</template>
