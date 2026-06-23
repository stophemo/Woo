<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { showToast } from 'vant'
import { getById } from '../../src/services/documentApi'
import type { DocumentDTO } from '../../src/services/documentApi'

const route = useRoute()
const router = useRouter()
const noteId = route.params.id as string
const title = ref('')
const content = ref('')
const loading = ref(true)

onMounted(async () => {
  try {
    const doc: DocumentDTO = await getById(noteId)
    title.value = doc.title
    content.value = doc.content ?? ''
  } catch {
    showToast('加载失败')
  } finally {
    loading.value = false
  }
})

function goBack() {
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
    />
    <div class="editor-content" v-html="content" />
    <van-loading v-if="loading" class="loading" />
  </div>
</template>

<style scoped>
.editor-content {
  padding: 16px;
  line-height: 1.6;
  font-size: 16px;
  word-wrap: break-word;
  overflow-wrap: break-word;
}
.loading {
  display: flex;
  justify-content: center;
  margin-top: 40px;
}
</style>
