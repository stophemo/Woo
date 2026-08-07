<template>
    <div 
        class="context-menu"
        :style="menuStyle"
        @click.stop
    >
        <template v-for="(item, index) in items" :key="`${item.action}-${index}`">
            <div v-if="item.divider" class="context-menu-divider"></div>
            <div
                v-else
                class="context-menu-item"
                :class="{ 'disabled': item.disabled }"
                @click="handleSelect(item)"
            >
                <span>{{ item.label }}</span>
                <span v-if="item.shortcut" class="context-menu-shortcut">{{ item.shortcut }}</span>
            </div>
        </template>
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
    const menuWidth = 220
    const menuHeight = Math.min(
        props.items.reduce((height, item) => height + (item.divider ? 9 : 36), 8),
        window.innerHeight - 20
    )
    
    let x = props.position.x
    let y = props.position.y
    
    // 检查右边界
    if (x + menuWidth > window.innerWidth) {
        x = Math.max(10, window.innerWidth - menuWidth - 10)
    }
    
    // 检查下边界
    if (y + menuHeight > window.innerHeight) {
        y = Math.max(10, window.innerHeight - menuHeight - 10)
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
const handleClickOutside = () => {
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
    background-color: var(--bg-elevated);
    border: 1px solid var(--border-primary);
    border-radius: 4px;
    box-shadow: var(--shadow-dropdown);
    padding: 4px 0;
    z-index: 9999;
    min-width: 220px;
    max-width: calc(100vw - 20px);
    max-height: calc(100vh - 20px);
    overflow-y: auto;
}

.context-menu-item {
    padding: 9px 12px 9px 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    cursor: pointer;
    font-size: 13px;
    color: var(--text-primary);
    transition: background-color 0.2s;
    -webkit-tap-highlight-color: transparent;
}

.context-menu-shortcut { color: var(--text-muted); font-size: 12px; white-space: nowrap; }
.context-menu-divider { height: 1px; margin: 4px 12px; background: var(--border-secondary); }

.context-menu-item:hover:not(.disabled) {
    background-color: var(--bg-hover);
}

.context-menu-item:active:not(.disabled) {
    background-color: var(--bg-active);
}

@media (max-width: 640px) {
    .context-menu {
        min-width: 180px;
        border-radius: 8px;
    }
    .context-menu-item {
        padding: 14px 18px;
        font-size: 14px;
    }
}

.context-menu-item.disabled {
    color: var(--text-disabled);
    cursor: not-allowed;
}
</style>
