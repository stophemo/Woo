<template>
    <div 
        class="context-menu"
        :style="menuStyle"
        @click.stop
    >
        <div 
            v-for="item in items" 
            :key="item.action"
            class="context-menu-item"
            :class="{ 'disabled': item.disabled }"
            @click="handleSelect(item)"
        >
            {{ item.label }}
        </div>
    </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount } from 'vue'
import type { ContextMenuPosition, ContextMenuItem } from '../../types/folder'

interface Props {
    position: ContextMenuPosition
    items: ContextMenuItem[]
}

interface Emits {
    (e: 'select', action: string): void
    (e: 'close'): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

// 计算菜单位置，确保不超出屏幕边界
const menuStyle = computed(() => {
    const menuWidth = 160
    const menuHeight = props.items.length * 36
    
    let x = props.position.x
    let y = props.position.y
    
    // 检查右边界
    if (x + menuWidth > window.innerWidth) {
        x = window.innerWidth - menuWidth - 10
    }
    
    // 检查下边界
    if (y + menuHeight > window.innerHeight) {
        y = window.innerHeight - menuHeight - 10
    }
    
    return {
        left: `${x}px`,
        top: `${y}px`
    }
})

const handleSelect = (item: ContextMenuItem) => {
    if (!item.disabled) {
        emit('select', item.action)
    }
}

// 点击其他地方关闭菜单
const handleClickOutside = (event: MouseEvent) => {
    emit('close')
}

onMounted(() => {
    setTimeout(() => {
        document.addEventListener('click', handleClickOutside)
    }, 0)
})

onBeforeUnmount(() => {
    document.removeEventListener('click', handleClickOutside)
})
</script>

<style scoped>
.context-menu {
    position: fixed;
    background-color: #fff;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    padding: 4px 0;
    z-index: 9999;
    min-width: 160px;
}

.context-menu-item {
    padding: 8px 16px;
    cursor: pointer;
    font-size: 13px;
    color: #333;
    transition: background-color 0.2s;
}

.context-menu-item:hover:not(.disabled) {
    background-color: #f0f0f0;
}

.context-menu-item.disabled {
    color: #999;
    cursor: not-allowed;
}
</style>
