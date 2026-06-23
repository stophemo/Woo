<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import { getById, updateContent, remove as deleteDoc } from '../../src/services/documentApi'
import type { DocumentDTO } from '../../src/services/documentApi'

const route = useRoute()
const router = useRouter()
const noteId = route.params.id as string

const title = ref('')
const content = ref('')
const loading = ref(true)
const saving = ref(false)
const editing = ref(false)
const editText = ref('')
let autoSaveTimer: ReturnType<typeof setTimeout> | null = null

onMounted(async () => {
  await loadNote()
})

async function loadNote() {
  try {
    const doc: DocumentDTO = await getById(noteId)
    title.value = doc.title
    content.value = doc.content ?? ''
  } catch {
    showToast('加载失败')
  } finally {
    loading.value = false
  }
}

function startEdit() {
  editText.value = content.value
  editing.value = true
}

function cancelEdit() {
  editing.value = false
  editText.value = ''
}

async function saveEdit() {
  saving.value = true
  try {
    await updateContent(noteId, editText.value)
    content.value = editText.value
    editing.value = false
    showToast('已保存')
  } catch {
    showToast('保存失败')
  } finally {
    saving.value = false
  }
}

async function confirmDelete() {
  try {
    await showConfirmDialog({ title: '删除笔记', message: '确定删除此笔记？' })
    await deleteDoc(noteId)
    showToast('已删除')
    router.back()
  } catch {
    // cancelled
  }
}

function goBack() {
  if (editing.value && editText.value !== content.value) {
    showConfirmDialog({ title: '放弃编辑？', message: '未保存的更改将丢失' })
      .then(() => router.back())
      .catch(() => {})
    return
  }
  router.back()
}

function stripHtml(html: string) {
  return html.replace(/<[^>]+>/g, '').trim()
}
</script>

<template>
  <div class="editor-page">
    <van-nav-bar
      :title="title || '无标题'"
      left-text="返回"
      left-arrow
      @click-left="goBack"
    >
      <template #right>
        <van-icon name="delete" class="action-icon" @click="confirmDelete" />
      </template>
    </van-nav-bar>

    <van-loading v-if="loading" class="loading" />

    <!-- 阅读模式 -->
    <template v-if="!loading && !editing">
      <div class="editor-content" v-html="content" />
      <div v-if="!content" class="empty-hint">暂无内容</div>
      <van-button
        type="primary"
        size="small"
        plain
        icon="edit"
        class="edit-btn"
        @click="startEdit"
      >
        编辑
      </van-button>
    </template>

    <!-- 编辑模式 -->
    <template v-if="editing">
      <van-field
        v-model="editText"
        type="textarea"
        autosize
        placeholder="在此输入 Markdown 或 HTML 内容..."
        class="edit-textarea"
      />
      <div class="edit-actions">
        <van-button size="small" @click="cancelEdit">取消</van-button>
        <van-button
          type="primary"
          size="small"
          :loading="saving"
          @click="saveEdit"
        >
          保存
        </van-button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.loading {
  display: flex;
  justify-content: center;
  margin-top: 40px;
}
.editor-content {
  padding: 16px;
  line-height: 1.6;
  font-size: 16px;
  word-wrap: break-word;
  overflow-wrap: break-word;
}
.editor-content :deep(h1) { font-size: 22px; margin: 16px 0 8px; }
.editor-content :deep(h2) { font-size: 19px; margin: 14px 0 6px; }
.editor-content :deep(h3) { font-size: 17px; margin: 12px 0 4px; }
.editor-content :deep(p) { margin: 8px 0; }
.editor-content :deep(ul), .editor-content :deep(ol) { padding-left: 20px; }
.editor-content :deep(blockquote) {
  border-left: 3px solid #1989fa;
  padding-left: 12px;
  color: #666;
  margin: 8px 0;
}
.empty-hint {
  text-align: center;
  color: #999;
  margin-top: 40px;
}
.edit-btn {
  position: fixed;
  right: 16px;
  bottom: 24px;
}
.edit-textarea {
  min-height: 200px;
  font-size: 14px;
  line-height: 1.6;
  --van-cell-vertical-padding: 12px;
}
.edit-actions {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  padding: 12px 16px;
}
.action-icon {
  font-size: 20px;
  padding: 4px;
}
</style>
