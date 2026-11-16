<script setup lang="ts">
/**
 * 移动端内联 SVG 图标。
 *
 * 应用内置的 Vant 图标字体是子集（vant-icon.woff2 只有 25KB），
 * `folder-o` / `edit-o` 等字形缺失，会渲染成空白。这里对缺失的字形
 * 用内联 SVG 兜底，同时线描风格也比字形图标更贴合纸面视觉。
 */
type IconName =
  | 'folder' | 'pencil' | 'doc'
  | 'undo' | 'redo' | 'keyboard' | 'link' | 'highlight'
  | 'quote' | 'divider' | 'code' | 'table'

withDefaults(defineProps<{ name: IconName; size?: number }>(), {
  size: 18,
})
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 16 16"
    fill="none"
    aria-hidden="true"
    class="m-icon"
  >
    <template v-if="name === 'folder'">
      <path
        d="M1.9 4.35a1 1 0 0 1 1-1h2.9c.3 0 .58.13.77.36l.66.8c.19.23.47.36.77.36h5.1a1 1 0 0 1 1 1v6.1a1 1 0 0 1-1 1H2.9a1 1 0 0 1-1-1V4.35Z"
        stroke="currentColor"
        stroke-width="1.15"
        stroke-linejoin="round"
      />
    </template>

    <template v-else-if="name === 'pencil'">
      <path
        d="M11.1 2.4a1.2 1.2 0 0 1 1.7 0l.8.8a1.2 1.2 0 0 1 0 1.7l-6.7 6.7-2.7.7.7-2.7 6.2-7.2Z"
        stroke="currentColor"
        stroke-width="1.15"
        stroke-linejoin="round"
      />
      <path d="M3.2 13.6h9.6" stroke="currentColor" stroke-width="1.15" stroke-linecap="round" />
    </template>

    <template v-else-if="name === 'undo'">
      <path d="M6.3 3.7 3 7l3.3 3.3" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M3.3 7h5.2a3.7 3.7 0 0 1 0 7.4H6.2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
    </template>

    <template v-else-if="name === 'redo'">
      <path d="M9.7 3.7 13 7l-3.3 3.3" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M12.7 7H7.5a3.7 3.7 0 0 0 0 7.4h2.3" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
    </template>

    <template v-else-if="name === 'keyboard'">
      <rect x="1.7" y="5.1" width="12.6" height="6.1" rx="1.1" stroke="currentColor" stroke-width="1.15" />
      <path d="M4.1 7.3h.01M6.2 7.3h.01M8.3 7.3h.01M10.4 7.3h.01M12.1 7.3h.01M5.2 9.3h5.6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
    </template>

    <template v-else-if="name === 'link'">
      <path d="m6.5 9.5 3-3" stroke="currentColor" stroke-width="1.15" stroke-linecap="round" />
      <path d="M7.1 4.9 8.5 3.5a2.35 2.35 0 0 1 3.3 3.3l-1.4 1.4" stroke="currentColor" stroke-width="1.15" stroke-linecap="round" />
      <path d="m8.9 11.1-1.4 1.4a2.35 2.35 0 0 1-3.3-3.3l1.4-1.4" stroke="currentColor" stroke-width="1.15" stroke-linecap="round" />
    </template>

    <template v-else-if="name === 'highlight'">
      <path d="M3.2 12.1 9.5 5.8l1.6 1.6-5.7 5.7-2.2.6.6-2.2Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
      <path d="M9.1 4.3 10.4 3a1 1 0 0 1 1.4 0l1.2 1.2a1 1 0 0 1 0 1.4l-1.3 1.3" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
    </template>

    <template v-else-if="name === 'quote'">
      <path d="M3.1 6.3c0-1.3.9-2.3 2.3-2.5v1.8c-.5.1-.8.4-.8.9h1v3.3H3.1V6.3Z" fill="currentColor" />
      <path d="M8.8 6.3c0-1.3.9-2.3 2.3-2.5v1.8c-.5.1-.8.4-.8.9h1v3.3H8.8V6.3Z" fill="currentColor" />
    </template>

    <template v-else-if="name === 'divider'">
      <path d="M2.6 8h10.8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
    </template>

    <template v-else-if="name === 'code'">
      <path d="m5.6 4.8-3.2 3.2 3.2 3.2M10.4 4.8l3.2 3.2-3.2 3.2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
    </template>

    <template v-else-if="name === 'table'">
      <rect x="2.2" y="3.1" width="11.6" height="9.8" rx="1" stroke="currentColor" stroke-width="1.1" />
      <path d="M2.2 6.5h11.6M2.2 9.8h11.6M6.1 3.1v9.8M9.9 3.1v9.8" stroke="currentColor" stroke-width="1.05" />
    </template>

    <template v-else>
      <path
        d="M9.2 1.9H4.1a1 1 0 0 0-1 1v10.2a1 1 0 0 0 1 1h7.8a1 1 0 0 0 1-1V5.5L9.2 1.9Z"
        stroke="currentColor"
        stroke-width="1.15"
        stroke-linejoin="round"
      />
      <path d="M9 2.2v3.2h3.4" stroke="currentColor" stroke-width="1.15" stroke-linejoin="round" />
      <path d="M5.4 8.6h5.2M5.4 11h3.6" stroke="currentColor" stroke-width="1.15" stroke-linecap="round" />
    </template>
  </svg>
</template>

<style scoped>
.m-icon {
  display: block;
}
</style>
