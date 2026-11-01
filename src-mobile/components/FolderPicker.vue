<script setup lang="ts">
/**
 * 目录选择弹层：把「移动到目录」这类操作在移动端做成一次点击可达。
 * 目录按层级缩进展示，空目录树时给出明确提示。
 */
import { computed } from 'vue'
import type { FolderNode } from '../../src/types/folder'
import { useWorkspaceStore } from '../../src/stores/workspace'

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

      <div v-if="flatFolders.length" class="picker-list">
        <button
          v-for="folder in flatFolders"
          :key="folder.id"
          type="button"
          class="picker-item"
          :style="{ paddingLeft: `${14 + folder.depth * 16}px` }"
          @click="pick(folder)"
        >
          <van-icon name="folder-o" class="picker-icon" />
          <span class="picker-name">{{ folder.name }}</span>
        </button>
      </div>

      <van-empty v-else description="还没有目录，先去首页新建一个" image-size="72" />
    </div>
  </van-popup>
</template>

<style scoped>
.picker {
  padding: 14px 0 calc(12px + env(safe-area-inset-bottom, 0px));
}

.picker-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 16px 8px;
}

.picker-head h3 {
  margin: 0;
  color: var(--van-text-color);
  font-size: 16px;
  font-weight: 650;
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
  color: var(--van-text-color-2);
  font-size: 17px;
}

.picker-list {
  max-height: 52vh;
  overflow-y: auto;
  padding: 4px 8px;
}

.picker-item {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 9px;
  padding: 12px 14px;
  border: 0;
  border-radius: 10px;
  background: transparent;
  text-align: left;
}

.picker-item:active {
  background: color-mix(in srgb, var(--van-primary-color) 10%, transparent);
}

.picker-icon {
  flex: 0 0 auto;
  color: var(--van-primary-color);
  font-size: 17px;
}

.picker-name {
  overflow: hidden;
  color: var(--van-text-color);
  font-size: 14px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
