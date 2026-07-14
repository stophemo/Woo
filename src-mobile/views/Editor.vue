<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'
import { useLockStore } from '../../src/stores/lock'

const route = useRoute()
const router = useRouter()
const store = useWorkspaceStore()
const lockStore = useLockStore()
const noteId = route.params.id as string

const loading = ref(true)
const saving = ref(false)
const editing = ref(false)
const editText = ref('')

// 统一从 store 读取当前文档（草稿走 localStorage，正式文档走后端）
const doc = computed(() => store.currentDocument)
const title = computed(() => doc.value?.title || '无标题')
const content = computed(() => doc.value?.content || '')
const isLocked = computed(() => !!doc.value?.isLocked)

// 解锁
const unlockPwd = ref('')
const unlocking = ref(false)
async function doUnlock() {
  if (!unlockPwd.value) return
  unlocking.value = true
  try {
    const ok = await lockStore.verify(unlockPwd.value)
    if (!ok) { showToast('密码错误'); return }
    await lockStore.unlockDocument(noteId)
    await store.selectDocument(noteId) // 重新加载，已解锁后含正文
    unlockPwd.value = ''
    showToast('已解锁')
  } finally {
    unlocking.value = false
  }
}

onMounted(async () => {
  try {
    await store.selectDocument(noteId)
  } catch {
    showToast('加载失败')
  } finally {
    loading.value = false
  }
})

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
    // 经 store：草稿写 localStorage，正式文档 800ms 防抖 + flush 强制落库
    store.updateDocumentContent(noteId, editText.value)
    await store.flushPendingSave()
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
    await store.deleteDocument(noteId)
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
</script>

<template>
  <div class="editor-page">
    <van-nav-bar
      :title="title"
      left-text="返回"
      left-arrow
      @click-left="goBack"
    >
      <template #right>
        <van-icon name="delete" class="action-icon" @click="confirmDelete" />
      </template>
    </van-nav-bar>

    <van-loading v-if="loading" class="loading" />

    <!-- 加密遮罩 -->
    <template v-else-if="isLocked">
      <div class="locked-box">
        <van-icon name="lock" size="40" color="#ff976a" />
        <p class="locked-title">此笔记已加密</p>
        <van-field v-model="unlockPwd" type="password" placeholder="输入密码锁密码" class="locked-field" />
        <van-button type="primary" round :loading="unlocking" @click="doUnlock">解锁查看</van-button>
      </div>
    </template>

    <!-- 阅读模式 -->
    <template v-else-if="!editing">
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

    <!-- 编辑模式（过渡：Markdown/HTML 源码，富文本为子项目 6） -->
    <template v-else>
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
.locked-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 60px 32px;
  gap: 16px;
}
.locked-title {
  color: #646566;
  margin: 0;
}
.locked-field {
  border: 1px solid #ebedf0;
  border-radius: 8px;
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
