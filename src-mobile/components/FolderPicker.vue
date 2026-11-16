<script setup lang="ts">
/**
 * 目录选择弹层：把「移动到目录」这类操作在移动端做成一次点击可达。
 * 目录按层级缩进展示，空目录树时给出明确提示。
 */
import { computed } from 'vue'
import type { FolderNode } from '../../src/types/folder'
import { useWorkspaceStore } from '../../src/stores/workspace'
import MIcon from './MIcon.vue'

const props = defineProps<{ show: boolean; title?: string }>()
const emit = defineEmits<{
  'update:show': [value: boolean]
  select: [folderId: string]
}>()

const store = useWorkspaceStore()

interface FlatFolder {
  id: string
  name: string
  depth: number
}

const flatFolders = computed<FlatFolder[]>(() => {
  const result: FlatFolder[] = []
  const walk = (nodes: FolderNode[], depth: number) => {
    for (const node of nodes) {
      result.push({ id: node.id, name: node.name, depth })
      if (node.children.length) walk(node.children, depth + 1)
    }
  }
  walk(store.folders, 0)
  return result
})

function close() {
  emit('update:show', false)
}

function pick(folder: FlatFolder) {
  emit('select', folder.id)
  close()
}
</script>

<template>
  <van-popup
    :show="props.show"
    position="bottom"
    round
    close-on-popstate
    :style="{ maxHeight: '72vh' }"
    @update:show="emit('update:show', $event)"
  >
    <div class="picker">
      <div class="picker-head">
        <h3>{{ props.title || '移动到目录' }}</h3>
        <button type="button" class="picker-close" aria-label="关闭" @click="close">
          <van-icon name="cross" />
        </button>
      </div>

      <ul v-if="flatFolders.length" class="w-group picker-list">
        <li v-for="folder in flatFolders" :key="folder.id">
          <button type="button" class="w-row" @click="pick(folder)">
            <span class="w-row__icon" :style="{ marginLeft: `${folder.depth * 14}px` }">
              <MIcon name="folder" :size="17" />
            </span>
            <span class="w-row__main">
              <span class="w-row__title">{{ folder.name }}</span>
            </span>
            <van-icon name="arrow" class="w-row__chev" />
          </button>
        </li>
      </ul>

      <van-empty v-else description="还没有目录，先去首页新建一个" image-size="64" />
    </div>
  </van-popup>
</template>

<style scoped>
.picker {
  padding: 14px 0 16px;
  background: var(--w-paper);
}

.picker-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 16px 10px;
}

.picker-head h3 {
  margin: 0;
  color: var(--w-ink);
  font-size: 16px;
  font-weight: 600;
}

.picker-close {
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--w-ink-2);
  font-size: 17px;
}

.picker-list {
  max-height: 52vh;
  overflow-y: auto;
  margin: 0 12px;
}
</style>
