<script setup lang="ts">
/**
 * 编辑字体设置。
 *
 * 与编辑器 Aa 面板共用同一份设置：选择即时生效并持久化。
 * 这里给一个独立入口，避免用户只在写作面板里才能找到字体。
 */
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import {
  editorFontId,
  editorFontOptions,
  setEditorFontId,
  editorFontSizeId,
  editorFontSizeOptions,
  editorFontSizePx,
  setEditorFontSizeId,
} from '../../src/config/editorFonts'

const router = useRouter()

const currentFont = computed(() =>
  editorFontOptions.find(option => option.id === editorFontId.value) ?? editorFontOptions[0],
)

function goBack() {
  if (window.history.length > 1) router.back()
  else router.replace('/settings')
}
</script>

<template>
  <div class="font-page">
    <header class="w-head font-head">
      <div class="w-head__row">
        <button type="button" class="w-icon-btn" aria-label="返回" @click="goBack">
          <van-icon name="arrow-left" />
        </button>
        <div class="w-head__titles">
          <span class="w-head__kicker">编辑</span>
          <h1 class="w-head__title">字体</h1>
        </div>
      </div>
      <p class="w-head__meta">只影响正文，不改变应用界面</p>
    </header>

    <div class="font-preview-card">
      <span class="font-preview-card__label">{{ currentFont.label }} · 正文预览</span>
      <p class="font-preview-card__title" :style="{ fontFamily: currentFont.stack }">春江潮水连海平</p>
      <p
        class="font-preview-card__text"
        :style="{ fontFamily: currentFont.stack, fontSize: `${editorFontSizePx}px` }"
      >
        海上明月共潮生。把长文、列表、引用和标题都交给这套字体，读起来更像一本自己的书。
      </p>
    </div>

    <div class="font-list">
      <button
        v-for="option in editorFontOptions"
        :key="option.id"
        type="button"
        class="font-option"
        :class="{ 'is-active': editorFontId === option.id }"
        @click="setEditorFontId(option.id)"
      >
        <span class="font-option__preview" :style="{ fontFamily: option.stack }">{{ option.preview }}</span>
        <span class="font-option__meta">
          <span class="font-option__name">{{ option.label }}</span>
          <span class="font-option__hint">{{ option.hint }}</span>
        </span>
        <span v-if="editorFontId === option.id" class="font-option__check" aria-hidden="true">✓</span>
      </button>
    </div>

    <div class="font-size-section">
      <p class="font-size-label">字号</p>
      <div class="font-size-segment">
        <button
          v-for="option in editorFontSizeOptions"
          :key="option.id"
          type="button"
          class="font-size-option"
          :class="{ 'is-active': editorFontSizeId === option.id }"
          @click="setEditorFontSizeId(option.id)"
        >{{ option.label }}</button>
      </div>
    </div>

    <p class="font-note">字号与编辑器「Aa」面板共用；字体和字号都只影响正文。</p>
  </div>
</template>

<style scoped>
.font-page {
  min-height: 100vh;
  min-height: 100dvh;
  padding-bottom: calc(28px + env(safe-area-inset-bottom, 0px));
  background: var(--w-paper);
}

.font-head .w-head__titles {
  padding-left: 2px;
}

.font-head .w-icon-btn {
  margin-left: -4px;
}

.font-preview-card {
  margin: 14px 16px 0;
  padding: 18px 18px 16px;
  border-radius: 16px;
  background: var(--w-canvas);
  box-shadow: var(--w-shadow), inset 0 0 0 1px var(--w-line);
}

.font-preview-card__label {
  color: var(--w-ink-3);
  font-size: 10.5px;
  font-weight: 650;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.font-preview-card__title {
  margin: 12px 0 6px;
  color: var(--w-ink);
  font-size: 24px;
  line-height: 1.4;
}

.font-preview-card__text {
  margin: 0;
  color: var(--w-ink-2);
  font-size: 15px;
  line-height: 1.8;
}

.font-list {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin: 16px 16px 0;
}

.font-option {
  position: relative;
  display: flex;
  min-height: 78px;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 12px 14px;
  border: 1px solid var(--w-line);
  border-radius: 14px;
  background: var(--w-canvas);
  color: var(--w-ink);
  text-align: left;
}

.font-option.is-active {
  border-color: color-mix(in srgb, var(--w-accent) 48%, var(--w-line));
  background: color-mix(in srgb, var(--w-accent) 6%, var(--w-canvas));
}

.font-option__preview {
  max-width: 100%;
  overflow: hidden;
  font-size: 18px;
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.font-option__meta {
  display: flex;
  align-items: baseline;
  gap: 7px;
}

.font-option__name {
  color: var(--w-ink);
  font-size: 13px;
  font-weight: 600;
}

.font-option__hint {
  color: var(--w-ink-3);
  font-size: 10.5px;
}

.font-option.is-active .font-option__name {
  color: var(--w-accent);
}

.font-option__check {
  position: absolute;
  top: 10px;
  right: 12px;
  color: var(--w-accent);
  font-size: 12px;
  font-weight: 700;
}

.font-size-section {
  margin: 18px 16px 0;
}

.font-size-label {
  margin: 0 0 8px;
  color: var(--w-ink-3);
  font-size: 10.5px;
  font-weight: 650;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.font-size-segment {
  display: flex;
  overflow: hidden;
  border: 1px solid var(--w-line);
  border-radius: 12px;
  background: var(--w-canvas);
}

.font-size-option {
  min-height: 42px;
  flex: 1;
  border: 0;
  border-right: 1px solid var(--w-line);
  background: transparent;
  color: var(--w-ink-2);
  font-size: 13px;
}

.font-size-option:last-child {
  border-right: 0;
}

.font-size-option.is-active {
  background: color-mix(in srgb, var(--w-accent) 8%, var(--w-canvas));
  color: var(--w-accent);
  font-weight: 650;
}

.font-note {
  margin: 16px 20px 0;
  color: var(--w-ink-3);
  font-size: 11.5px;
  line-height: 1.7;
}
</style>
