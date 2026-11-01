<script setup lang="ts">
/**
 * 移动端内联 SVG 图标。
 *
 * 应用内置的 Vant 图标字体是子集（vant-icon.woff2 只有 25KB），
 * `folder-o` / `edit-o` 等字形缺失，会渲染成空白。这里对缺失的几个字形
 * 用内联 SVG 兜底，同时线描风格也比字形图标更贴合纸面视觉。
 */
withDefaults(defineProps<{ name: 'folder' | 'pencil' | 'doc'; size?: number }>(), {
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
