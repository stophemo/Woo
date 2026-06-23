<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { getTree } from '../../src/services/folderApi'
import { listByFolder } from '../../src/services/documentApi'
import type { FolderTreeNodeDTO } from '../../src/services/folderApi'
import type { DocumentDTO } from '../../src/services/documentApi'

const router = useRouter()
const folders = ref<FolderTreeNodeDTO[]>([])
const documents = ref<DocumentDTO[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    folders.value = await getTree()
  } catch (e) {
    showToast('加载失败')
  } finally {
    loading.value = false
  }
})

async function onFolderClick(folderId: string) {
  loading.value = true
  try {
    documents.value = await listByFolder(folderId)
  } finally {
    loading.value = false
  }
}

function openNote(id: string) {
  router.push(`/note/${id}`)
}

function formatTime(ts: string) {
  return new Date(ts).toLocaleDateString('zh-CN')
}
</script>

<template>
  <div class="home-page">
    <van-pull-refresh v-model="loading" @refresh="onMounted">
      <van-cell-group title="文件夹">
        <van-cell
          v-for="f in folders"
          :key="f.id"
          :title="f.name"
          is-link
          @click="onFolderClick(f.id)"
        />
        <van-empty v-if="!folders.length" description="暂无文件夹" />
      </van-cell-group>

      <van-cell-group v-if="documents.length" title="文档">
        <van-cell
          v-for="d in documents"
          :key="d.id"
          :title="d.title"
          :label="formatTime(d.updateTime)"
          @click="openNote(d.id)"
        />
      </van-cell-group>
    </van-pull-refresh>
  </div>
</template>
