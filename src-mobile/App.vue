<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'

const router = useRouter()
const route = useRoute()

const TAB_ROUTES = ['/', '/drafts', '/settings']
const active = ref(TAB_ROUTES.indexOf(route.path) >= 0 ? TAB_ROUTES.indexOf(route.path) : 0)

watch(() => route.path, (path) => {
  const idx = TAB_ROUTES.indexOf(path)
  if (idx >= 0) active.value = idx
})

function onTabChange(index: number) {
  router.push(TAB_ROUTES[index])
}
</script>

<template>
  <div class="mobile-app">
    <div class="page-content">
      <router-view />
    </div>
    <van-tabbar v-model="active" @change="onTabChange" safe-area-inset-bottom>
      <van-tabbar-item icon="notes-o">笔记</van-tabbar-item>
      <van-tabbar-item icon="edit-o">草稿</van-tabbar-item>
      <van-tabbar-item icon="setting-o">设置</van-tabbar-item>
    </van-tabbar>
  </div>
</template>

<style>
body {
  margin: 0;
  padding: 0;
  background: #f7f8fa;
}
.mobile-app {
  min-height: 100vh;
  background: #f7f8fa;
}
.page-content {
  padding-bottom: 50px;
}
</style>
